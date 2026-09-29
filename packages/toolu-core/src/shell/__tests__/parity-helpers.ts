/**
 * Shared by the bash-parity suites (#284): run the unmodified shipped bash libs
 * over fixture commands, and compose the same answers from `@toolu/core/shell`.
 * The bash side only reads `plugins/toolu/hooks/lib/detect.sh`; nothing under
 * `plugins/` is written. `bash_commands_decide` is answered by the native gate
 * only (#261); its bash baselines stay recorded in the fixtures.
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { z } from "zod";
import {
  isGitCommit,
  isGitPush,
  pushTargetBranch,
  pushTargetRoot,
} from "../../detect/detect-git.ts";
import { bashCommandsDecide } from "../../gates/bash-commands.ts";
import { analyzeShell } from "../shell-parse.ts";
import { writeTargets } from "../shell-writes.ts";

export const REPO = resolve(import.meta.dir, "../../../../..");
export const FIXTURES = join(REPO, "tooling/fixtures/shell");
const DETECT_SH = join(REPO, "plugins/toolu/hooks/lib/detect.sh");

export type Env = Record<string, string>;

/** A clean environment: no host-session variable leaks into the bash side. */
export function cleanEnv(home: string, extra: Env = {}): Env {
  return { PATH: process.env.PATH ?? "/usr/bin:/bin", HOME: home, LC_ALL: "C", ...extra };
}

/**
 * Source `lib`, then run `body` once per NUL-separated record on stdin. Each
 * record's fields are read into `$f1`, `$f2`, …; each result ends with a NUL.
 */
export function bashBatch(
  lib: "detect",
  body: string,
  records: readonly (readonly string[])[],
  env: Env,
  cwd = REPO,
): string[] {
  const fields = records[0]?.length ?? 1;
  const reads = Array.from(
    { length: fields },
    (_, i) => `IFS= read -r -d '' f${i + 1} || break`,
  ).join("; ");
  const source = { detect: DETECT_SH }[lib];
  const script = `. "${source}"\nwhile :; do ${reads}; ${body}; printf '\\0'; done`;
  const stdin = records.map((record) => record.map((field) => `${field}\0`).join("")).join("");
  const res = spawnSync("bash", ["-c", script], {
    cwd,
    input: stdin,
    env,
    encoding: "utf8",
    maxBuffer: 64 << 20,
  });
  if (res.status !== 0) throw new Error(`bash exited ${String(res.status)}: ${res.stderr}`);
  return res.stdout.split("\0").slice(0, records.length);
}

export const DecideCase = z.object({
  command: z.string(),
  allow: z.array(z.string()),
  deny: z.array(z.string()),
});
export type DecideCase = z.infer<typeof DecideCase>;

/** `is_git_push` / `is_git_commit` through the production detect layer (#254). */
export function tsIsGit(command: string, sub: "push" | "commit"): boolean {
  const analysis = analyzeShell(command);
  return sub === "push" ? isGitPush(analysis) : isGitCommit(analysis);
}

/** Write targets as `bash_write_targets` prints them: static paths, else the text as written. */
export function tsWriteTargets(command: string): string[] {
  return writeTargets(analyzeShell(command)).map((t) => t.path ?? t.text);
}

/** `bash_commands_decide`'s answer from the production gate (#261), in bash's `allow` / `deny:<rule>` form. */
export function tsDecide(c: DecideCase): string {
  const verdict = bashCommandsDecide(analyzeShell(c.command), c);
  if (verdict.kind === "unknown") return `unknown:${verdict.why}`;
  return verdict.kind === "allow" ? "allow" : `deny:${verdict.rule}`;
}

/** `push_target_root` through the production detect layer (#254). */
export function tsPushRoot(command: string, cwd: string, env: Env): string {
  return pushTargetRoot(analyzeShell(command), { env, cwd });
}

/** `push_target_branch` through the production detect layer (#254). */
export function tsPushBranch(command: string, root: string, env: Env): string {
  return pushTargetBranch(analyzeShell(command), root, env);
}

/** A disposable directory tree, removed by `using`. */
export function scratch(prefix: string): { dir: string; [Symbol.dispose](): void } {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), prefix)));
  return { dir, [Symbol.dispose]: () => rmSync(dir, { recursive: true, force: true }) };
}

export function initRepo(dir: string, branch = "main"): void {
  mkdirSync(dir, { recursive: true });
  const git = (...args: string[]) => spawnSync("git", ["-C", dir, ...args], { encoding: "utf8" });
  git("init", "-q", "-b", branch);
  git("-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init");
}

export function relativeTo(base: string, path: string): string {
  return relative(base, realpathSync(path)) || ".";
}

export function readFixture(name: string): unknown {
  return JSON.parse(readFileSync(join(FIXTURES, name), "utf8"));
}
