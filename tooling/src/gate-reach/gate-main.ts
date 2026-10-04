/** The shared exit contract of the gate-reach entries: 0 clean, 1 findings, 3 misconfigured. */
import { resolve } from "node:path";
import { envOr } from "../env.ts";
import { GateFatal } from "./reach-schema.ts";

/**
 * Run `check` over the repo (or `rootEnv`'s tree), one finding per stderr
 * line. Every failure to finish is exit 3: an uncaught error would exit 1, which
 * means findings.
 */
export function gateMain(
  label: string,
  rootEnv: string,
  check: (root: string) => string[],
): number {
  const root = envOr(rootEnv, resolve(import.meta.dir, "../../.."));
  try {
    const findings = check(root);
    for (const finding of findings) console.error(finding);
    return findings.length === 0 ? 0 : 1;
  } catch (err: unknown) {
    const detail = err instanceof Error ? (err.stack ?? err.message) : String(err);
    console.error(
      `${label}: ${err instanceof GateFatal ? err.message : `unexpected failure: ${detail}`}`,
    );
    return 3;
  }
}
