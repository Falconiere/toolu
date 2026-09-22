import { mkdtempSync } from "node:fs";
import { join } from "node:path";
import { bootstrapWithNoOpRunner } from "@toolu/opencode/bootstrap";
import { listPluginManifests } from "@toolu/opencode/inventory";
import type { SuiteOutcome } from "../types.ts";
import { isolatedHome, repoRoot, tmpBase } from "./helpers.ts";

/** Exit-0 bootstrap without artifacts → NotReady (#211 / #212). */
export async function runBootstrapReadinessSuite(): Promise<SuiteOutcome> {
  const root = repoRoot();
  const project = mkdtempSync(join(tmpBase(), "toolu-conformance-bs-"));
  const dataRoot = mkdtempSync(join(tmpBase(), "toolu-conformance-bs-data-"));
  const pluginsRoot = join(root, "plugins");
  const manifests = listPluginManifests(pluginsRoot);
  if (manifests === null) {
    return { status: "fail", message: `cannot read plugins root: ${pluginsRoot}` };
  }
  const toolu = manifests.find((m) => m.name === "toolu");
  if (!toolu) {
    return { status: "fail", message: "toolu plugin manifest missing under plugins/" };
  }

  const result = await bootstrapWithNoOpRunner({
    repoRoot: root,
    projectRoot: project,
    dataRoot,
    plugins: [toolu],
    isolatedHome: isolatedHome("toolu-conf-home-bs-"),
  });

  if (result.status === "not-ready") {
    return { status: "pass" };
  }

  return {
    status: "fail",
    message: `expected not-ready from no-op bootstrap runner, got ${result.status}`,
  };
}
