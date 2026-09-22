import { mkdirSync, mkdtempSync } from "node:fs";
import { join } from "node:path";
import type { SuiteOutcome } from "../types.ts";
import { installProtectedProjectAt, tmpBase } from "./helpers.ts";
import { runProtectedEditBridge } from "./protected-bridge.ts";

/** Project path containing spaces still blocks protected .env edit (#212). */
export async function runSpacesCwdSuite(): Promise<SuiteOutcome> {
  const spacedParent = join(tmpBase(), "toolu conf spaces");
  mkdirSync(spacedParent, { recursive: true });
  const projectRoot = mkdtempSync(join(spacedParent, "proj-"));
  const { envPath } = installProtectedProjectAt(projectRoot);

  return runProtectedEditBridge({
    projectRoot,
    envPath,
    failPrefix: "spaces-cwd",
  });
}
