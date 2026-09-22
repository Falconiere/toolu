import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { buildInventory, lookupPluginInstallState } from "../scan.ts";
import { resolveEnabledPluginNames } from "../selection.ts";
import { opencodePluginSelectionPath, opencodeProjectConfigPath } from "../../host/roots.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";

function repoRoot(): string {
  return join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
}

function writeManifest(pluginsRoot: string, name: string): void {
  const dir = join(pluginsRoot, name, ".claude-plugin");
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "plugin.json"),
    JSON.stringify({ name, version: "1.0.0", dependencies: [] }),
  );
}

test("inventory install states installed absent unknown", () => {
  const pluginsRoot = join(repoRoot(), "plugins");
  expect(lookupPluginInstallState(pluginsRoot, "toolu")).toBe("installed");
  expect(lookupPluginInstallState(pluginsRoot, "no-such-plugin-ever")).toBe("absent");
  expect(lookupPluginInstallState("/nonexistent/plugins-root", "toolu")).toBe("unknown");
});

test("enabled selection from .opencode/toolu/plugins.json", () => {
  const project = mkdtempSync(join(tmpBase, "toolu-inv-en-"));
  const pluginsRoot = mkdtempSync(join(tmpBase, "toolu-inv-pl-"));
  writeManifest(pluginsRoot, "toolu");
  writeManifest(pluginsRoot, "ts-quality");
  mkdirSync(join(project, ".opencode", "toolu"), { recursive: true });
  writeFileSync(
    opencodePluginSelectionPath(project),
    JSON.stringify({ version: 1, enabled: ["toolu"] }),
  );
  const result = resolveEnabledPluginNames(pluginsRoot, project);
  expect(result.ok).toBe(true);
  if (!result.ok) {
    return;
  }
  expect(result.enabled.has("toolu")).toBe(true);
  expect(result.enabled.has("ts-quality")).toBe(false);
  const inventory = buildInventory(pluginsRoot, result.enabled);
  const tsEntry = inventory?.find((e) => e.name === "ts-quality");
  expect(tsEntry).toBeDefined();
  expect(tsEntry?.enabled).toBe("disabled");
});

test("skills false in toolu.config disables plugin", () => {
  const project = mkdtempSync(join(tmpBase, "toolu-inv-sk-"));
  const pluginsRoot = mkdtempSync(join(tmpBase, "toolu-inv-sk-pl-"));
  writeManifest(pluginsRoot, "toolu");
  mkdirSync(join(project, ".opencode"), { recursive: true });
  writeFileSync(
    opencodeProjectConfigPath(project),
    JSON.stringify({ version: 1, skills: { toolu: false } }),
  );
  const result = resolveEnabledPluginNames(pluginsRoot, project);
  expect(result.ok).toBe(true);
  if (!result.ok) {
    return;
  }
  expect(result.enabled.has("toolu")).toBe(false);
});
