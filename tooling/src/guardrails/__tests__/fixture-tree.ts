/**
 * Builds a guardrails fixture tree that is a REAL git repository (the upstream
 * kit's mkrepo.sh): copy tooling/fixtures/guardrails/<name>, write the
 * runtime-only files, then `git add -A` and commit. `.gitignore` decides what
 * gets tracked, which is what makes the clean and violating trees differ for
 * the secrets check.
 *
 * Node built-ins only: the one-time golden capture ran this module inside a bare
 * container to drive the bash baseline, with no node_modules available.
 */
import { spawnSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

export const FIXTURES = resolve(import.meta.dir, "../../../fixtures/guardrails");

export type FixtureName = "clean" | "violating" | "workspace" | "workspace-violating";

export type FixtureTree = {
  readonly root: string;
  write(rel: string, body: string): void;
  [Symbol.dispose](): void;
};

/** runtime-files.json → { "<fixture>/<rel>": body } decoded from base64. */
function runtimeFiles(): Map<string, string> {
  const doc: unknown = JSON.parse(readFileSync(join(FIXTURES, "runtime-files.json"), "utf8"));
  const files: unknown =
    typeof doc === "object" && doc !== null ? Reflect.get(doc, "files") : undefined;
  if (typeof files !== "object" || files === null) {
    throw new Error("runtime-files.json has no files map");
  }
  const out = new Map<string, string>();
  for (const [rel, encoded] of Object.entries(files)) {
    if (typeof encoded !== "string") throw new Error(`runtime-files.json: ${rel} is not base64`);
    out.set(rel, Buffer.from(encoded, "base64").toString("utf8"));
  }
  return out;
}

function writeAt(abs: string, body: string): void {
  mkdirSync(dirname(abs), { recursive: true });
  writeFileSync(abs, body);
}

function git(root: string, args: string[]): void {
  const res = spawnSync("git", ["-C", root, ...args], { encoding: "utf8" });
  if (res.error) throw res.error;
  if (res.status !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr.trim()}`);
}

/** A fresh copy of fixture `name`, committed in its own git repo; `using` removes it. */
export function buildFixture(name: FixtureName, extra: Record<string, string> = {}): FixtureTree {
  const root = realpathSync(mkdtempSync(join(tmpdir(), `gr-${name}-`)));
  cpSync(join(FIXTURES, name), root, { recursive: true });
  for (const [rel, body] of runtimeFiles()) {
    if (rel.startsWith(`${name}/`)) writeAt(join(root, rel.slice(name.length + 1)), body);
  }
  for (const [rel, body] of Object.entries(extra)) writeAt(join(root, rel), body);
  git(root, ["init", "-q", "-b", "main"]);
  git(root, ["add", "-A"]);
  git(root, [
    "-c",
    "user.email=fixture@example.com",
    "-c",
    "user.name=fixture",
    "-c",
    "commit.gpgsign=false",
    "commit",
    "-qm",
    "fixture",
  ]);
  return {
    root,
    write: (rel, body) => writeAt(join(root, rel), body),
    [Symbol.dispose]: () => rmSync(root, { recursive: true, force: true }),
  };
}

/** Every file in a built tree, repo-relative, sorted, `.git` excluded. */
export function listFiles(root: string, dir = ""): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(join(root, dir), { withFileTypes: true })) {
    const rel = dir === "" ? entry.name : `${dir}/${entry.name}`;
    if (entry.name === ".git") continue;
    if (entry.isDirectory()) out.push(...listFiles(root, rel));
    else if (entry.isFile()) out.push(rel);
  }
  return out.toSorted();
}
