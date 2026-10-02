import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { selectPluginsWithDependencies } from "../resolve.ts";
import {
  opencodeGlobalPluginSelectionPath,
  opencodePluginSelectionPath,
} from "../../host/roots.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";

function writeManifest(
  pluginsRoot: string,
  name: string,
  dependencies: Array<{ name: string; marketplace: string }> = [],
): void {
  const dir = join(pluginsRoot, name, ".claude-plugin");
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "plugin.json"), JSON.stringify({ name, version: "1.0.0", dependencies }));
}

test("select fails closed on missing dependency", () => {
  const project = mkdtempSync(join(tmpBase, "toolu-sel-proj-"));
  const pluginsRoot = mkdtempSync(join(tmpBase, "toolu-sel-pl-"));
  writeManifest(pluginsRoot, "ts-quality", [{ name: "toolu", marketplace: "toolu" }]);
  mkdirSync(join(project, ".opencode", "toolu"), { recursive: true });
  writeFileSync(
    opencodePluginSelectionPath(project),
    JSON.stringify({ version: 1, enabled: ["ts-quality"] }),
  );
  const result = selectPluginsWithDependencies(pluginsRoot, project);
  expect(result.ok).toBe(false);
  if (result.ok) {
    return;
  }
  expect(result.missingDependency).toBe("toolu");
});

test("select includes dependency closure", () => {
  const project = mkdtempSync(join(tmpBase, "toolu-sel-cl-"));
  const pluginsRoot = mkdtempSync(join(tmpBase, "toolu-sel-cl-pl-"));
  writeManifest(pluginsRoot, "toolu");
  writeManifest(pluginsRoot, "ts-quality", [{ name: "toolu", marketplace: "toolu" }]);
  mkdirSync(join(project, ".opencode", "toolu"), { recursive: true });
  writeFileSync(
    opencodePluginSelectionPath(project),
    JSON.stringify({ version: 1, enabled: ["ts-quality"] }),
  );
  const result = selectPluginsWithDependencies(pluginsRoot, project);
  expect(result.ok).toBe(true);
  if (!result.ok) {
    return;
  }
  expect(result.plugins.map((p) => p.name).toSorted()).toEqual(["toolu", "ts-quality"]);
});

test("selection source and not-installed notes reach the closure result (#345)", () => {
  const project = mkdtempSync(join(tmpBase, "toolu-sel-src-"));
  const pluginsRoot = mkdtempSync(join(tmpBase, "toolu-sel-src-pl-"));
  const global = mkdtempSync(join(tmpBase, "toolu-sel-src-gl-"));
  writeManifest(pluginsRoot, "toolu");
  writeManifest(pluginsRoot, "ts-quality", [{ name: "toolu", marketplace: "toolu" }]);
  const globalPath = opencodeGlobalPluginSelectionPath(global);
  mkdirSync(join(global, "toolu"), { recursive: true });
  writeFileSync(globalPath, JSON.stringify({ version: 1, enabled: ["ts-quality", "gone"] }));
  const result = selectPluginsWithDependencies(pluginsRoot, project, global);
  if (!result.ok) throw new Error(result.reason);
  expect(result.source).toBe("global");
  expect(result.plugins.map((p) => p.name)).toEqual(["ts-quality", "toolu"]);
  expect(result.notes).toEqual([`enabled plugin "gone" in ${globalPath} is not installed`]);
});

test("an invalid explicit selection makes the closure fail with the file's reason (#345)", () => {
  const project = mkdtempSync(join(tmpBase, "toolu-sel-bad-"));
  const pluginsRoot = mkdtempSync(join(tmpBase, "toolu-sel-bad-pl-"));
  writeManifest(pluginsRoot, "toolu");
  mkdirSync(join(project, ".opencode", "toolu"), { recursive: true });
  writeFileSync(opencodePluginSelectionPath(project), "{ broken");
  const result = selectPluginsWithDependencies(pluginsRoot, project);
  expect(result.ok).toBe(false);
  if (!result.ok)
    expect(result.reason).toStartWith(`invalid ${opencodePluginSelectionPath(project)}: `);
});
