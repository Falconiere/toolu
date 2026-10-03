/** The shared exit contract of the gate-reach entries: 0 clean, 1 findings, 3 misconfigured. */
import { resolve } from "node:path";
import { envOr } from "../env.ts";
import { GateFatal } from "./reach-schema.ts";

/** Run `check` over the repo (or `rootEnv`'s tree), one finding per stderr line. */
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
    if (!(err instanceof GateFatal)) throw err;
    console.error(`${label}: ${err.message}`);
    return 3;
  }
}
