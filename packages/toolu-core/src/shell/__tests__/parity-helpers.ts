/**
 * Fixture helpers for the native shell analyzer and its gate consumers.
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
export const FIXTURES = join(REPO, "fixtures/shell");

export type Env = Record<string, string>;

/** A clean environment with no host-session variable leaks. */
export function cleanEnv(home: string, extra: Env = {}): Env {
  return { PATH: process.env.PATH ?? "/usr/bin:/bin", HOME: home, LC_ALL: "C", ...extra };
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
