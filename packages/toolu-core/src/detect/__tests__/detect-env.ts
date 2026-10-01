/** An isolated environment for native detection tests. */
import { childEnv, type EnvPatch } from "@toolu/conformance/harness/spawn";

export function detectEnv(home: string, extra: EnvPatch = {}): Record<string, string> {
  return childEnv({ HOME: home, LC_ALL: "C", ...extra });
}
