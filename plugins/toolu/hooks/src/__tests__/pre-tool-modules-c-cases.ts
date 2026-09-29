/**
 * Case shape and runner for the native push-review, plan-ledger, docs-sync and
 * agent-tier gates (#262). Every `@test` of their deleted bats suites is a case
 * with the same input and intent. Each case runs the whole hook in a fresh git
 * sandbox and records what the host sees (stdout, stderr, exit code) and every
 * file the hook added, changed or removed (telemetry lines, pending waivers),
 * timestamps normalised. The base PreToolUse bundle and `agent-tier.sh` at
 * 2912cd9d produced `fixtures/pre-tool-modules-c-golden.json`, before the bash
 * modules were deleted; `pre-tool-modules-c.test.ts` replays the new bundles.
 */
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative } from "node:path";
import { bashFixture, toStdin, type Fixture } from "@toolu/conformance/harness/fixtures";
import { pretoolEnv, TOOLU_PLUGIN, type PretoolHost } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { launcherCommand } from "@toolu/core/launcher";
import type { Outcome } from "./pre-tool-modules-b-cases.ts";

export type Hook = "pre-tools" | "agent-tier";

export type GateCase = {
  name: string;
  host: PretoolHost;
  hook: Hook;
  /** Repos, commits, state files; runs after the sandbox exists, before the call. */
  setup?: (sb: Sandbox) => void;
  /** A Bash command, or a whole fixture. */
  command?: string;
  fixture?: (sb: Sandbox) => Fixture;
  /** Raw stdin, e.g. malformed JSON. Wins over `command` and `fixture`. */
  stdin?: string;
  /** Project `toolu.config.json` for the case's host. */
  config?: object | string;
  /** The hook's working directory, default the project. */
  cwd?: (sb: Sandbox) => string;
  env?: (sb: Sandbox) => EnvPatch;
  /** Tools left off PATH, e.g. `["jq"]`. */
  without?: string[];
  expect: Outcome;
  /** Substrings the decision text must contain, and must not. */
  has?: string[];
  lacks?: string[];
  /**
   * Why bash answered differently: a #283 defect it had, or a contract of the
   * TypeScript core. Its golden result is kept as the known-wrong baseline.
   */
  deviation?: string;
};

export type CaseInput = Omit<GateCase, "host" | "hook"> & { host?: PretoolHost; hook?: Hook };

/** A case builder with the group's defaults filled in. */
export function group(defaults: Partial<GateCase>): (c: CaseInput) => GateCase {
  return (c) => ({ host: "claude", hook: "pre-tools", ...defaults, ...c });
}

/** A hook call's result plus the files it touched, `null` for a removed file. */
export type Captured = {
  stdout: string;
  stderr: string;
  exitCode: number;
  files: Record<string, string | null>;
};

/** The shipped base branch every sandbox starts on. */
export const BASE = "main";
export const FEATURE = "feat/example";

/**
 * The bats `setup_sandbox`: `base.txt` on the base branch, then `feat/example`
 * with one commit of `feature.txt`. `dir` defaults to the project.
 */
export function featureRepo(sb: Sandbox, dir: string = sb.project): void {
  commitFile(sb, "base.txt", "base", dir);
  gitIn(dir, ["checkout", "-q", "-b", FEATURE]);
  commitFile(sb, "feature.txt", "feature", dir);
}

/** Write `body` at `path` under `dir`, parents created. */
export function writeIn(dir: string, path: string, body: string): void {
  const abs = join(dir, path);
  mkdirSync(dirname(abs), { recursive: true });
  writeFileSync(abs, body);
}

export function gitIn(dir: string, args: readonly string[]): string {
  const res = spawnSync("git", ["-C", dir, ...args], { encoding: "utf8" });
  if (res.status !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr.trim()}`);
  return res.stdout;
}

/** Commit `path` (parents created) in `dir`, default the project. */
export function commitFile(sb: Sandbox, path: string, body = "x", dir = sb.project): void {
  writeIn(dir, path, `${body}\n`);
  gitIn(dir, ["add", path]);
  gitIn(dir, ["commit", "-q", "-m", `add ${path}`]);
}

/** `git diff --no-color BASE...HEAD | git hash-object --stdin`, the gates' diff sha. */
export function diffShaIn(dir: string, base: string = BASE): string {
  const diff = spawnSync("git", ["-C", dir, "diff", "--no-color", `${base}...HEAD`]);
  const hash = spawnSync("git", ["-C", dir, "hash-object", "--stdin"], { input: diff.stdout });
  return hash.stdout.toString().trim();
}

/** `git diff BASE...HEAD --name-only` in `dir`. */
export function changedFiles(dir: string, base: string = BASE): string[] {
  return gitIn(dir, ["diff", "--no-color", `${base}...HEAD`, "--name-only"])
    .split("\n")
    .filter((line) => line !== "");
}

/** The branch slug bash derives: `/` → `_`, then only `[A-Za-z0-9_-]`. */
export function slugOf(branch: string): string {
  const slug = branch.replaceAll("/", "_").replace(/[^A-Za-z0-9_-]/g, "");
  return slug === "" ? "_default" : slug;
}

export function headBranch(dir: string): string {
  return gitIn(dir, ["rev-parse", "--abbrev-ref", "HEAD"]).trim();
}

/** Every tool the hooks and their libs reach for. */
const TOOLS = [
  "bash",
  "sh",
  "git",
  "awk",
  "grep",
  "sed",
  "tr",
  "cat",
  "head",
  "tail",
  "dirname",
  "basename",
  "mktemp",
  "rm",
  "mkdir",
  "env",
  "uname",
  "cut",
  "sort",
  "uniq",
  "wc",
  "date",
  "printf",
  "ls",
  "find",
  "xargs",
  "readlink",
  "realpath",
  "stat",
  "touch",
  "mv",
  "cp",
  "ln",
  "tee",
  "sleep",
  "id",
  "od",
  "shasum",
  "chmod",
  "cmp",
  "diff",
  "comm",
  "paste",
  "expr",
  "python3",
  "jq",
];

/** A PATH of symlinks to every tool in `TOOLS` except `without`. */
function pathWithout(root: string, without: readonly string[]): string {
  const bin = join(root, "bin-path");
  mkdirSync(bin, { recursive: true });
  for (const tool of TOOLS.filter((t) => !without.includes(t))) {
    const found = Bun.which(tool);
    if (found !== null) symlinkSync(found, join(bin, tool));
  }
  return bin;
}

/** Paths under the sandbox root that are not hook state. */
function ignored(rel: string): boolean {
  const parts = rel.split("/");
  if (parts.includes(".git")) return true;
  if (parts[0] === "settings" || parts[0]?.startsWith("bin-") === true) return true;
  // Bun and other tools cache under HOME; only the host config dirs are state.
  return parts[0] === "home" && parts[1] !== ".claude" && parts[1] !== ".codex";
}

/** Every file under the sandbox root that could be hook state, by relative path. */
function snapshot(root: string, dir: string = root, out = new Map<string, string>()) {
  for (const entry of readdirSync(dir)) {
    const abs = join(dir, entry);
    const rel = relative(root, abs);
    if (ignored(rel)) continue;
    const stat = statSync(abs, { throwIfNoEntry: false });
    if (stat?.isDirectory() === true) snapshot(root, abs, out);
    else if (stat?.isFile() === true) out.set(rel, readFileSync(abs, "utf8"));
  }
  return out;
}

const ISO = /\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ/g;

function touched(
  before: Map<string, string>,
  after: Map<string, string>,
  norm: (text: string) => string,
): Record<string, string | null> {
  const files: Record<string, string | null> = {};
  for (const [rel, body] of after) {
    if (before.get(rel) !== body) files[rel] = norm(body).replace(ISO, "<time>");
  }
  for (const rel of before.keys()) if (!after.has(rel)) files[rel] = null;
  return files;
}

/** How a side spawns `hook` for plugin root `root`. */
export type Argv = (hook: Hook, root: string) => string[];

/** The committed bundle behind its generated launcher. */
export const bundleArgv: Argv = (hook) => [
  "/bin/sh",
  "-c",
  launcherCommand({ plugin: "toolu", event: "PreToolUse", entry: hook }),
];

/** Run `c` in a fresh sandbox with the plugin at `root`, the sandbox root normalised to `$ROOT`. */
export async function runCase(
  c: GateCase,
  argv: Argv = bundleArgv,
  root: string = TOOLU_PLUGIN,
): Promise<Captured> {
  using sb = createSandbox({ git: true, branch: BASE });
  sb.git("config", "maintenance.auto", "false");
  sb.git("config", "gc.auto", "0");
  const settings = join(sb.root, "settings");
  mkdirSync(settings, { recursive: true });
  if (c.config !== undefined) {
    const body = typeof c.config === "string" ? c.config : JSON.stringify(c.config);
    writeIn(sb.configDir(c.host, "project"), "toolu.config.json", body);
  }
  c.setup?.(sb);
  const cwd = c.cwd?.(sb) ?? sb.project;
  const stdin =
    c.stdin ??
    JSON.stringify(toStdin(c.host, c.fixture?.(sb) ?? bashFixture(c.command ?? ""), { cwd }));
  const path = c.without === undefined ? {} : { PATH: pathWithout(sb.root, c.without) };
  const env = pretoolEnv(sb, c.host, {
    TOOLU_SETTINGS_DIR: settings,
    CLAUDE_PLUGIN_ROOT: root,
    ...(c.host === "codex" ? { PLUGIN_ROOT: root } : {}),
    ...path,
    ...c.env?.(sb),
  });
  const before = snapshot(sb.root);
  const result = await run(argv(c.hook, root), { cwd, env, stdin });
  const norm = (text: string) => text.split(sb.root).join("$ROOT");
  return {
    stdout: norm(result.stdout),
    stderr: norm(result.stderr),
    exitCode: result.exitCode,
    files: touched(before, snapshot(sb.root), norm),
  };
}

/** What must match: the decision JSON (not its whitespace), exit code, stderr, touched files. */
export function comparable(result: Captured): Omit<Captured, "stdout"> & { stdout: unknown } {
  const stdout: unknown = result.stdout.trim() === "" ? "" : JSON.parse(result.stdout);
  return { ...result, stdout };
}
