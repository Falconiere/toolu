/** Single-repo mode: one guardrails.config.json at the working directory. */
import type { Options } from "./cli.ts";
import { packageContext, runFiles, runRepo, shellPwd, unchangedTree } from "./package-run.ts";
import { editedPath, readStdin, stopHookActive } from "./stdin.ts";
import { exists } from "./walk.ts";

export function runSingle(cwd: string, opts: Options): number {
  const env = process.env;
  const configFile = env["GR_CONFIG"] === undefined || env["GR_CONFIG"] === "" ? "guardrails.config.json" : env["GR_CONFIG"];
  const ctx = packageContext(cwd, env["GR_PATH_PREFIX"] ?? "", configFile);

  if (opts.mode === "repo") {
    runRepo(ctx, opts.only);
    return ctx.report.failed ? 1 : 0;
  }
  if (opts.mode === "file") {
    runFiles(ctx, opts.only, opts.paths, true);
    return ctx.report.failed ? 1 : 0;
  }
  const payload = readStdin();
  if (opts.mode === "hook") {
    // An Edit/Write that touched nothing we can resolve is not an error.
    let edited = editedPath(payload);
    if (edited === "") return 0;
    const pwd = `${shellPwd(cwd)}/`;
    if (edited.startsWith(pwd)) edited = edited.slice(pwd.length);
    if (!exists(cwd, edited)) return 0;
    runFiles(ctx, opts.only, [edited], false);
    return ctx.report.failed ? 2 : 0;
  }
  // Stop: already continuing from a block, or nothing changed this turn.
  if (stopHookActive(payload) || unchangedTree(cwd)) return 0;
  runRepo(ctx, opts.only);
  return ctx.report.failed ? 2 : 0;
}
