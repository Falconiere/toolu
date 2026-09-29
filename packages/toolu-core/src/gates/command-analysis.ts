/**
 * One shell parse per post-tool event for the gates (#259). gate-status and
 * push-waiver both read the command a Bash/Shell call ran; the first to ask
 * pays for `analyzeShell` and the second gets the same object. The `WeakMap`
 * lives as long as the event does.
 */
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { analyzeShell } from "../shell/shell-parse.ts";
import type { ShellAnalysis } from "../shell/shell-types.ts";
import { toolCommand } from "./tool-exit.ts";

const analyses = new WeakMap<RegistryHookEvent, ShellAnalysis>();

/** Whether the tool is a shell: Cursor Agent names it `Shell`, Claude Code `Bash`. */
export function isShellTool(event: RegistryHookEvent): boolean {
  return event.toolName === "Bash" || event.toolName === "Shell";
}

/** The analysis of the command the payload reports, as the bash modules read it. */
export function commandAnalysis(event: RegistryHookEvent, ctx: RegistryContext): ShellAnalysis {
  const cached = analyses.get(event);
  if (cached !== undefined) return cached;
  const analysis = analyzeShell(toolCommand(ctx.raw));
  analyses.set(event, analysis);
  return analysis;
}
