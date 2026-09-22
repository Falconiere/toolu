import type { SuiteOutcome } from "../types.ts";
import { createProtectedProject } from "./helpers.ts";
import { runProtectedEditBridge } from "./protected-bridge.ts";

/** Protected .env edit via Claude bridge → deny|ask; bytes unchanged on deny (#212 AC-2). */
export async function runProtectedFilesSuite(): Promise<SuiteOutcome> {
  const { projectRoot, envPath, envBefore } = createProtectedProject("toolu-conformance-pf-");
  return runProtectedEditBridge({
    projectRoot,
    envPath,
    envBefore,
    failPrefix: "protected-files",
  });
}
