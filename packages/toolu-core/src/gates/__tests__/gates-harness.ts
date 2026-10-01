/** Runs native PostToolUse modules against an isolated real repository. */
import { projectState } from "@toolu/conformance/harness/posttool";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { dispatchPostTool, type ToolModule } from "../../dispatch/dispatch.ts";
import { LIB } from "../../dispatch/__tests__/dispatch-harness.ts";

export type Side = { stdout: string; exitCode: number; state: Record<string, string> };

export type Call = { stdin: string; env: Record<string, string> };

/** Project state plus that of `also` (other repositories the calls may write to). */
export type Watch = { also?: readonly string[] };

/** Run `calls` in order and return the last output plus the final state. */
export async function runNative(
  sb: Sandbox,
  module: ToolModule,
  calls: readonly Call[],
  watch: Watch = {},
): Promise<Side> {
  let out = { stdout: "", stderr: "", exitCode: 0 };
  for (const call of calls) {
    out = await dispatchPostTool(call.stdin, {
      builtins: [module],
      libDir: LIB,
      env: call.env,
      cwd: sb.project,
    });
  }
  return { stdout: out.stdout, exitCode: out.exitCode, state: projectState(sb, watch.also) };
}

/** A Claude Code PostToolUse payload for a Bash call. */
export function bashPayload(command: string, response: Record<string, unknown>): string {
  return JSON.stringify({ tool_name: "Bash", tool_input: { command }, tool_response: response });
}
