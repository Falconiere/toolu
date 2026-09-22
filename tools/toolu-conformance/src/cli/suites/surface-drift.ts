import type { SuiteOutcome } from "../types.ts";
import { repoRoot } from "./helpers.ts";
import { runArgvCheck } from "./spawn-check.ts";

/** Committed OpenCode surface matches generated output (#206 / #212). */
export async function runSurfaceDriftSuite(): Promise<SuiteOutcome> {
  return runArgvCheck(["bun", "run", "check:opencode-surface"], {
    cwd: repoRoot(),
    failLabel: "check:opencode-surface",
  });
}
