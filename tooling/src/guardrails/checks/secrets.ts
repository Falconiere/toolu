/**
 * secrets — a secrets file (.dev.vars, .env) must never be tracked by git.
 * Once committed it is in the history forever, so this catches the one moment
 * it can still be cheap to fix. Reads `git ls-files`, so it needs a real repo.
 */
import type { RepoFacts } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import { git, inGitRepo } from "../git.ts";

export function secrets(ctx: CheckContext<RepoFacts>, mode: Mode): void {
  if (mode !== "repo" || !inGitRepo(ctx.root)) return;
  for (const target of ctx.config.secretFiles) {
    if (git(ctx.root, ["ls-files", "--error-unmatch", target]).status === 0) {
      ctx.report.violation(
        "secrets",
        target,
        "tracked by git — it holds secrets",
        `run: git rm --cached ${target} && echo ${target} >> .gitignore (and rotate anything already pushed)`,
      );
    }
  }
}
