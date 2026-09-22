/** argv-only bash runner for the portable bridge (#210). */
import { execBashRun } from "./runner-exec.ts";
import type { BashRunArgs, BashRunner, RawProcessResult } from "./runner-types.ts";

export type {
  BashRunArgs,
  BashRunner,
  RawProcessFailureCode,
  RawProcessResult,
} from "./runner-types.ts";

/** Bun.spawn runner: argv array only, never a shell string. */
export function createBunBashRunner(): BashRunner {
  return {
    run(args: BashRunArgs): Promise<RawProcessResult> {
      return execBashRun(args);
    },
  };
}
