/**
 * The bash side of the detect parity suites (#254): source the unmodified
 * `plugins/toolu/hooks/lib/detect.sh` and run one snippet, asynchronously, so
 * concurrent tests do not block each other. Nothing under `plugins/` is written.
 */
import { expect } from "bun:test";
import { resolve } from "node:path";
import { childEnv, run, type EnvPatch } from "@toolu/conformance/harness/spawn";

export const DETECT_SH = resolve(
  import.meta.dir,
  "../../../../../plugins/toolu/hooks/lib/detect.sh",
);

/** One environment for both sides: the parent's minus host-session keys, with `HOME` isolated. */
export function detectEnv(home: string, extra: EnvPatch = {}): Record<string, string> {
  return childEnv({ HOME: home, LC_ALL: "C", ...extra });
}

/** Stdout of `body` run after sourcing detect.sh, with `$1…` bound to `args`. */
export async function bashDetect(
  body: string,
  args: readonly string[],
  cwd: string,
  env: Record<string, string>,
): Promise<string> {
  const res = await run(["bash", "-c", `. "$0"; ${body}`, DETECT_SH, ...args], { cwd, env });
  expect(res.stderr).toBe("");
  expect(res.exitCode).toBe(0);
  return res.stdout;
}
