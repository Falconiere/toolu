/**
 * One package (or a single-repo project) run: load its config, then run the
 * selected checks in the documented order. Checks never chdir; every path is
 * relative to `root`, and `prefix` puts a workspace package back in the output.
 */
import { spawnSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { patterns } from "./checks/patterns.ts";
import { loadConfig } from "./config.ts";
import type { GuardrailsConfig } from "./config.ts";
import type { CheckContext } from "./context.ts";
import { git } from "./git.ts";
import { FILE_CHECKS, ROOT_CHECKS, TREE_CHECKS, selected } from "./registry.ts";
import { Reporter } from "./report.ts";
import { isFile } from "./walk.ts";

export function packageContext(
  root: string,
  prefix: string,
  configFile: string,
): CheckContext<GuardrailsConfig> {
  const config = loadConfig(root, configFile, isFile(root, configFile));
  return { root, config, report: new Reporter(prefix) };
}

/** Repo mode: every selected check over the whole tree. */
export function runRepo(ctx: CheckContext<GuardrailsConfig>, only: string): void {
  const owned = ctx.config.ownedByLinter;
  for (const [id, check] of FILE_CHECKS) if (selected(id, owned, only)) check(ctx, "repo", "");
  if (selected("patterns", owned, only)) patterns(ctx, "repo", []);
  for (const [id, check] of TREE_CHECKS) if (selected(id, owned, only)) check(ctx, "repo", "");
  for (const [id, check] of ROOT_CHECKS) if (selected(id, owned, only)) check(ctx, "repo");
}

/** File mode: per-path checks once per path, then the batch check once for all. */
export function runFiles(
  ctx: CheckContext<GuardrailsConfig>,
  only: string,
  paths: readonly string[],
  withBatch: boolean,
): void {
  const owned = ctx.config.ownedByLinter;
  for (const path of paths) {
    for (const [id, check] of FILE_CHECKS) if (selected(id, owned, only)) check(ctx, "file", path);
  }
  if (withBatch && selected("patterns", owned, only)) patterns(ctx, "batch", paths);
}

/** `git status --porcelain` is empty inside a work tree: nothing changed this turn. */
export function unchangedTree(root: string): boolean {
  if (git(root, ["rev-parse", "--git-dir"]).status !== 0) return false;
  const res = spawnSync("git", ["status", "--porcelain"], { cwd: root, encoding: "utf8" });
  return res.stdout === "";
}

/**
 * The working directory as bash's $PWD: the inherited PWD when it names this
 * directory, else the physical path. Hook payloads carry absolute paths.
 */
export function shellPwd(cwd: string): string {
  const inherited = process.env["PWD"];
  if (inherited !== undefined && inherited !== "") {
    try {
      if (realpathSync(inherited) === realpathSync(cwd)) return inherited;
    } catch {
      return cwd;
    }
  }
  return cwd;
}
