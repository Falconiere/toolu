/**
 * The bash→TypeScript parity cases (#277). The same list drives both sides:
 * the one-time capture ran the bash runner from origin/main inside a Linux
 * container (GNU grep) and wrote tooling/fixtures/guardrails/golden.json;
 * golden-parity.test.ts replays every case against the TypeScript runner.
 *
 * Output is compared as a sorted multiset of lines with the tree root
 * replaced by <ROOT>: bash fanned workspace packages out in parallel, so its
 * line order was never part of the contract.
 */
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildFixture, listFiles } from "./fixture-tree.ts";
import type { FixtureName } from "./fixture-tree.ts";

export const CHECK_IDS = [
  "folder-tree",
  "file-size",
  "colocated-tests",
  "no-barrels",
  "filename-case",
  "secret-content",
  "lint-suppressions",
  "patterns",
  "folder-readmes",
  "test-tree",
  "banned-deps",
  "shadow-configs",
  "required-files",
  "secrets",
] as const;

export type GoldenCase = {
  readonly id: string;
  readonly fixture: FixtureName;
  readonly args: readonly string[];
  /** Hook payload; `<ROOT>` is replaced by the tree root before the run. */
  readonly stdin: string;
  /** Files written after the commit, making the tree dirty. */
  readonly mutate?: Readonly<Record<string, string>>;
};

export type GoldenResult = { exit: number; stdout: string[]; stderr: string[] };

const SINGLE: readonly FixtureName[] = ["clean", "violating"];
const WORKSPACES: readonly FixtureName[] = ["workspace", "workspace-violating"];
const NEW_FILE = "export const fresh = 1;\n";

function hookPayload(path: string): string {
  return JSON.stringify({ tool_name: "Edit", tool_input: { file_path: path } });
}

function perFileCases(fixture: FixtureName, files: readonly string[]): GoldenCase[] {
  return files.flatMap((file) => [
    { id: `${fixture} --file ${file}`, fixture, args: ["--file", file], stdin: "" },
    { id: `${fixture} --hook ${file}`, fixture, args: ["--hook"], stdin: hookPayload(`<ROOT>/${file}`) },
  ]);
}

function stopCases(fixture: FixtureName, newFile: string): GoldenCase[] {
  return [
    { id: `${fixture} --stop active`, fixture, args: ["--stop"], stdin: '{"stop_hook_active":true}' },
    { id: `${fixture} --stop unchanged`, fixture, args: ["--stop"], stdin: "{}" },
    { id: `${fixture} --stop dirty`, fixture, args: ["--stop"], stdin: "{}", mutate: { [newFile]: NEW_FILE } },
  ];
}

function commonCases(fixture: FixtureName, files: readonly string[]): GoldenCase[] {
  return [
    { id: `${fixture} repo`, fixture, args: [], stdin: "" },
    { id: `${fixture} --list`, fixture, args: ["--list"], stdin: "" },
    { id: `${fixture} --file all`, fixture, args: ["--file", ...files], stdin: "" },
    { id: `${fixture} --hook empty payload`, fixture, args: ["--hook"], stdin: "{}" },
    { id: `${fixture} --hook not json`, fixture, args: ["--hook"], stdin: "not json" },
    { id: `${fixture} --hook missing file`, fixture, args: ["--hook"], stdin: hookPayload("<ROOT>/src/missing.ts") },
    { id: `${fixture} unknown flag`, fixture, args: ["--bogus"], stdin: "" },
    { id: `${fixture} --only without value`, fixture, args: ["--only"], stdin: "" },
    { id: `${fixture} --file without paths`, fixture, args: ["--file"], stdin: "" },
  ];
}

/** Every parity case, given each fixture's committed file list. */
export function goldenCases(filesOf: (fixture: FixtureName) => readonly string[]): GoldenCase[] {
  const cases: GoldenCase[] = [];
  for (const fixture of SINGLE) {
    const files = filesOf(fixture);
    cases.push(...commonCases(fixture, files), ...perFileCases(fixture, files));
    cases.push(...stopCases(fixture, "src/utilities/zz-new.ts"));
    for (const check of CHECK_IDS) {
      cases.push({ id: `${fixture} --only ${check}`, fixture, args: ["--only", check], stdin: "" });
    }
    cases.push({
      id: `${fixture} --file then --only`,
      fixture,
      args: ["--file", ...files, "--only", "file-size,secret-content,lint-suppressions"],
      stdin: "",
    });
  }
  for (const fixture of WORKSPACES) {
    const files = filesOf(fixture);
    cases.push(...commonCases(fixture, files), ...perFileCases(fixture, files));
    cases.push(...stopCases(fixture, "packages/api/src/utilities/zz-new.ts"));
    cases.push(
      { id: `${fixture} --only banned-deps`, fixture, args: ["--only", "banned-deps"], stdin: "" },
      { id: `${fixture} --only secrets`, fixture, args: ["--only", "secrets"], stdin: "" },
      { id: `${fixture} --hook outside root`, fixture, args: ["--hook"], stdin: hookPayload("/etc/hosts") },
    );
  }
  return cases;
}

/** File lists straight from freshly built trees (runtime files included). */
export function fixtureFiles(fixture: FixtureName): string[] {
  using tree = buildFixture(fixture);
  return listFiles(tree.root);
}

function lines(text: string, root: string): string[] {
  return text
    .split("\n")
    .filter((line) => line !== "")
    .map((line) => line.split(root).join("<ROOT>"))
    .toSorted();
}

/** Run one case with `argv0` (the runner command) in a tree built for it. */
export function runCase(argv0: readonly string[], c: GoldenCase): GoldenResult {
  using tree = buildFixture(c.fixture);
  for (const [rel, body] of Object.entries(c.mutate ?? {})) tree.write(rel, body);
  // HOME outside the tree: a runtime writing caches under $HOME (Bun on macOS
  // writes ~/Library) would otherwise make the tree look dirty to --stop.
  const home = mkdtempSync(join(tmpdir(), "gr-home-"));
  const [cmd = "", ...rest] = argv0;
  const res = spawnSync(cmd, [...rest, ...c.args], {
    cwd: tree.root,
    env: { PATH: process.env["PATH"] ?? "", HOME: home },
    input: c.stdin.split("<ROOT>").join(tree.root),
    encoding: "utf8",
  });
  rmSync(home, { recursive: true, force: true });
  if (res.error) throw res.error;
  return {
    exit: res.status ?? -1,
    stdout: lines(res.stdout, tree.root),
    stderr: lines(res.stderr, tree.root),
  };
}
