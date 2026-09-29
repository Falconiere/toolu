/**
 * Shared plumbing for the ledger parity suites (#256): run the unmodified bash
 * libs as real subprocesses, and keep only the stderr lines the libs author
 * themselves (decision D3: jq's own error text is not part of the contract).
 */
import { resolve } from "node:path";
import { run, type EnvPatch, type RunResult } from "@toolu/conformance/harness/spawn";

export const LIB = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib");

export type RunOpts = { cwd: string; env?: EnvPatch };

/** Source `lib` (a file in LIB), then call `fn args...` in the same shell. */
export function bashFn(lib: string, fn: string, args: string[], opts: RunOpts): Promise<RunResult> {
  return run(["bash", "-c", '. "$1"; shift; "$@"', "_", resolve(LIB, lib), fn, ...args], opts);
}

/** Source `lib`, then run `script` in the same shell with `args` as `$1...`. */
export function bashEval(
  lib: string,
  script: string,
  args: string[],
  opts: RunOpts,
): Promise<RunResult> {
  return run(["bash", "-c", `. "$1"; shift; ${script}`, "_", resolve(LIB, lib), ...args], opts);
}

/** Run `plan-ledger.sh`, `verdict.sh` or another lib as a CLI. */
export function bashCli(lib: string, args: string[], opts: RunOpts): Promise<RunResult> {
  return run(["bash", resolve(LIB, lib), ...args], opts);
}

const TAGGED = /^(plan-ledger|plan-ledger-parse|preflight|verdict)(:| --self-test:)/;

/** The lines the libs print themselves, in order; jq and git noise dropped. */
export function tagged(stderr: string): string[] {
  return stderr.split("\n").filter((line) => TAGGED.test(line));
}

/** What a parity case compares: exit code, exact stdout, and the tagged stderr lines. */
export type Outcome = { code: number; stdout: string; stderr: string[] };

export function bashOutcome(res: RunResult): Outcome {
  return { code: res.exitCode, stdout: res.stdout, stderr: tagged(res.stderr) };
}
