import type { SuiteOutcome } from "../types.ts";
import { runArgvCheck } from "./spawn-check.ts";

/** Optional live OpenCode CLI probe (#212 AC-4). */
export async function runLiveOpencodeSuite(): Promise<SuiteOutcome> {
  if (process.env.TOOLU_LIVE_OPENCODE !== "1") {
    return {
      status: "skip",
      message: "set TOOLU_LIVE_OPENCODE=1 to run live opencode --version probe",
    };
  }

  const bin = process.env.OPENCODE_BIN ?? "opencode";
  return runArgvCheck([bin, "--version"], {
    failLabel: `${bin} --version`,
  });
}
