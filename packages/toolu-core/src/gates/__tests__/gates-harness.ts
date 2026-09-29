/**
 * Runs one PostToolUse call two ways from the same sandbox state (#259): the
 * shipped bash module under `post-tools/mod.sh`'s dispatcher, and its native
 * port under `dispatchPostTool`. Returns each side's stdout and the state it
 * left in the project, timestamps normalised, so a test can compare them.
 */
import { join, resolve } from "node:path";
import { projectState } from "@toolu/conformance/harness/posttool";
import { fromSameState } from "@toolu/conformance/harness/pretool";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { dispatchPostTool, type ToolModule } from "../../dispatch/dispatch.ts";
import {
  LIB,
  modulesDir,
  runBashDispatch,
  writeModule,
} from "../../dispatch/__tests__/dispatch-harness.ts";

const MODULES = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/post-tools/modules");

export type Side = { stdout: string; exitCode: number; state: Record<string, string> };

export type Call = { stdin: string; env: Record<string, string> };

/** Project state plus that of `also` (other repositories the calls may write to). */
export type Watch = { also?: readonly string[] };

/**
 * Run `calls` in order through bash (module `<name>.sh` from the shipped
 * modules directory) and, from the same starting state, through the native
 * `module`. Each side reports the last call's output and the final state.
 */
export async function bothSides(
  sb: Sandbox,
  name: string,
  module: ToolModule,
  calls: readonly Call[],
  watch: Watch = {},
): Promise<{ bash: Side; ts: Side }> {
  const stateOf = (s: Sandbox) => projectState(s, watch.also);
  writeModule(modulesDir(sb), `${name}.sh`, `exec bash "${join(MODULES, `${name}.sh`)}"`);
  const [bash, ts] = await fromSameState(
    sb,
    () => {
      let out = { stdout: "", stderr: "", exitCode: 0 };
      for (const call of calls) out = runBashDispatch(sb, call.stdin, call.env, "post");
      return Promise.resolve({ ...out, state: stateOf(sb) });
    },
    async () => {
      let out = { stdout: "", stderr: "", exitCode: 0 };
      for (const call of calls) {
        out = await dispatchPostTool(call.stdin, {
          builtins: [module],
          libDir: LIB,
          env: call.env,
          cwd: sb.project,
        });
      }
      return { ...out, state: stateOf(sb) };
    },
  );
  return {
    bash: { stdout: bash.stdout, exitCode: bash.exitCode, state: bash.state },
    ts: { stdout: ts.stdout, exitCode: ts.exitCode, state: ts.state },
  };
}

/** A Claude Code PostToolUse payload for a Bash call. */
export function bashPayload(command: string, response: Record<string, unknown>): string {
  return JSON.stringify({ tool_name: "Bash", tool_input: { command }, tool_response: response });
}
