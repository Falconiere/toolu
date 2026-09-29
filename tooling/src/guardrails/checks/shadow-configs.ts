/**
 * shadow-configs — a config file that silently overrides another. Every entry
 * is a bug whose symptom is "the tool ran and did nothing" (lefthook.yaml
 * shadowed by the installer's stub, vitest.config.ts replacing vite.config.ts,
 * wrangler.toml beside wrangler.jsonc).
 */
import type { RepoFacts } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import { exists } from "../walk.ts";

export function shadowConfigs(ctx: CheckContext<RepoFacts>, mode: Mode): void {
  if (mode !== "repo") return;
  for (const { found, use, why } of ctx.config.shadowConfigs) {
    if (exists(ctx.root, found)) {
      ctx.report.violation(
        "shadow-configs",
        found,
        `shadows ${use} (${why})`,
        `delete ${found} and keep the configuration in ${use}`,
      );
    }
  }
}
