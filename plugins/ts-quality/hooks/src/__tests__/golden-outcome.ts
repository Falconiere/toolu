/** The decision class of one hook result, as the #259 parity suite classifies it. */
import { isJsonObject } from "@toolu/core/config";
import type { PostOutcome } from "@toolu/conformance/harness/posttool-corpus";

export function outcomeOf(stdout: string, exitCode: number): PostOutcome {
  if (exitCode === 2) return "exit2";
  if (stdout.trim() === "") return "silent";
  const out: unknown = JSON.parse(stdout);
  return isJsonObject(out) && out.decision === "block" ? "block" : "advisory";
}
