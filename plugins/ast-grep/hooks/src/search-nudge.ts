/**
 * ast-grep's PreToolUse registry module (#268): nudges a structural Grep
 * pattern, and grep/rg run in Bash to search files, toward ast-grep and the
 * Grep tool. Advisory only. ast-grep's state (config opt-out, then PATH) is
 * looked up only once a nudge is due, so most calls pay nothing for it.
 */
import { enabled, loadConfig } from "@toolu/core/config";
import { detectAstGrep } from "@toolu/core/detect";
import { defineRegistryModule, type RegistryContext } from "@toolu/core/registry";
import { shellAnalysisOf } from "@toolu/core/shell";
import {
  grepToolNudge,
  nudgeMessage,
  shellNudge,
  type AstGrepState,
  type NudgeKind,
} from "./lib/nudge-rules.ts";

function astGrepState(ctx: RegistryContext): AstGrepState {
  const where = { env: ctx.env, ...(ctx.cwd === undefined ? {} : { cwd: ctx.cwd }) };
  const config = loadConfig({ ...where, host: ctx.host, warn: () => undefined });
  if (!enabled(config, "skills", "ast-grep")) return "opt-out";
  return detectAstGrep(ctx.env) ? "available" : "missing";
}

export default defineRegistryModule({
  spec: "ast-grep@toolu",
  name: "search-nudge",
  event: "tool/pre",
  run(event, ctx) {
    let kind: NudgeKind | undefined;
    if (event.type === "shell/pre") kind = shellNudge(shellAnalysisOf(event));
    else if (event.toolName === "Grep") kind = grepToolNudge(event.toolInput);
    const message = kind === undefined ? undefined : nudgeMessage(kind, astGrepState(ctx));
    return Promise.resolve(
      message === undefined ? { kind: "allow" } : { kind: "advisory", message },
    );
  },
});
