/** Test helper: exit 0 without producing registry output → NotReady (#211 AC-4). */
import type { BashRunner, RawProcessResult } from "@toolu/core/runner";
import { bootstrapRuntime, type BootstrapRuntimeOptions } from "./runtime.ts";
import type { BootstrapResult } from "./result.ts";

export function createNoOpRunner(): BashRunner {
  return {
    run(): Promise<RawProcessResult> {
      return Promise.resolve({
        ok: true,
        exitCode: 0,
        stdout: "",
        stderr: "",
        truncated: false,
      });
    },
  };
}

/** Run bootstrap with a runner that always succeeds without touching disk scripts. */
export async function bootstrapWithNoOpRunner(
  options: BootstrapRuntimeOptions,
): Promise<BootstrapResult> {
  return bootstrapRuntime({ ...options, runner: createNoOpRunner() });
}
