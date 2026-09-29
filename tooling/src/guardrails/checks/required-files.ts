/**
 * required-files — files whose absence is silent rather than loud: a missing
 * 404 page is served as an empty 200, a missing wrangler.jsonc leaves the
 * Worker with no deploy config. Both fail in production, not at build time.
 */
import type { RepoFacts } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import { exists } from "../walk.ts";

export function requiredFiles(ctx: CheckContext<RepoFacts>, mode: Mode): void {
  if (mode !== "repo") return;
  for (const { path, why } of ctx.config.requiredFiles) {
    if (!exists(ctx.root, path)) {
      ctx.report.violation("required-files", path, "required file is missing", `create it — ${why}`);
    }
  }
}
