import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { bootstrapRuntime } from "@toolu/opencode/bootstrap";
import type { SuiteOutcome } from "../types.ts";
import { isolatedHome, repoRoot, tmpBase } from "./helpers.ts";

/** Exit-0 bootstrap without artifacts → NotReady (#211 / #212). */
export async function runBootstrapReadinessSuite(): Promise<SuiteOutcome> {
  const root = repoRoot();
  const project = mkdtempSync(join(tmpBase(), "toolu-conformance-bs-"));
  const dataRoot = mkdtempSync(join(tmpBase(), "toolu-conformance-bs-data-"));
  const pluginDir = mkdtempSync(join(tmpBase(), "toolu-conformance-empty-plugin-"));
  mkdirSync(join(pluginDir, "hooks", "dist"), { recursive: true });
  writeFileSync(join(pluginDir, "hooks", "dist", "register.js"), "process.exit(0);\n");

  const result = await bootstrapRuntime({
    repoRoot: root,
    projectRoot: project,
    dataRoot,
    plugins: [
      {
        name: "empty",
        spec: "empty@toolu",
        marketplace: "toolu",
        version: "1",
        pluginDir,
        dependencies: [],
      },
    ],
    isolatedHome: isolatedHome("toolu-conf-home-bs-"),
  });

  if (result.status === "not-ready") {
    return { status: "pass" };
  }

  return {
    status: "fail",
    message: `expected not-ready from successful bundle without artifacts, got ${result.status}`,
  };
}
