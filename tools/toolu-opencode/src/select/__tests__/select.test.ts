import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { selectPluginsWithDependencies } from "../resolve.ts";
import { opencodePluginSelectionPath } from "../../host/roots.ts";

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
