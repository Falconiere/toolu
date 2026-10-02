import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { launcherHook } from "@toolu/core/launcher";
import { bootstrapRuntime } from "@toolu/opencode/bootstrap";
import type { SuiteOutcome } from "../types.ts";
import { isolatedHome, repoRoot, tmpBase } from "./helpers.ts";

/** A data root an earlier session left: a readiness marker and an unrelated registry module. */
function staleDataRoot(): string {
  const dataRoot = mkdtempSync(join(tmpBase(), "toolu-conformance-bs-data-"));
  mkdirSync(join(dataRoot, "toolu", "pre-tools.d"), { recursive: true });
  writeFileSync(join(dataRoot, "toolu", ".session-start-ready"), "");
  writeFileSync(
    join(dataRoot, "toolu", "pre-tools.d", "custom@local__extra.js"),
    "export default {};\n",
  );
  return dataRoot;
}

/** A plugin whose hooks.json declares a register entry whose bundle is not there. */
function pluginMissingItsBundle(): string {
  const pluginDir = join(mkdtempSync(join(tmpBase(), "toolu-conformance-plugin-")), "empty");
  mkdirSync(join(pluginDir, "hooks", "dist"), { recursive: true });
  const hook = launcherHook({ plugin: "empty", event: "SessionStart", entry: "register" });
  writeFileSync(
    join(pluginDir, "hooks", "hooks.json"),
    JSON.stringify({ hooks: { SessionStart: [{ matcher: "startup", hooks: [hook] }] } }),
  );
  return pluginDir;
}

/** Leftover files cannot stand in for a selected plugin's startup (#211, #342). */
export async function runBootstrapReadinessSuite(): Promise<SuiteOutcome> {
  const project = mkdtempSync(join(tmpBase(), "toolu-conformance-bs-"));
  const result = await bootstrapRuntime({
    repoRoot: repoRoot(),
    projectRoot: project,
    dataRoot: staleDataRoot(),
    plugins: [
      {
        name: "empty",
        spec: "empty@toolu",
        marketplace: "toolu",
        version: "1",
        pluginDir: pluginMissingItsBundle(),
        dependencies: [],
      },
    ],
    isolatedHome: isolatedHome("toolu-conf-home-bs-"),
  });

  if (
    result.status === "not-ready" &&
    result.reason === "empty: register: missing startup bundle"
  ) {
    return { status: "pass" };
  }
  return {
    status: "fail",
    message: `expected not-ready for a missing declared startup bundle beside a stale marker and an unrelated module, got ${JSON.stringify(result)}`,
  };
}
