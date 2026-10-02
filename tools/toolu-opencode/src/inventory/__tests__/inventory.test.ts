import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { buildInventory, lookupPluginInstallState } from "../scan.ts";
import { resolveEnabledPluginNames } from "../selection.ts";
import {
  opencodeGlobalPluginSelectionPath,
  opencodePluginSelectionPath,
  opencodeProjectConfigPath,
} from "../../host/roots.ts";

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

/** A plugins root with toolu, jev and toolu-review, a project and a global config root (#345). */
function selectionFixture(): { pluginsRoot: string; project: string; global: string } {
  const pluginsRoot = mkdtempSync(join(tmpBase, "toolu-inv-sel-pl-"));
  for (const name of ["toolu", "jev", "toolu-review"]) writeManifest(pluginsRoot, name);
  const project = mkdtempSync(join(tmpBase, "toolu-inv-sel-proj-"));
  const global = mkdtempSync(join(tmpBase, "toolu-inv-sel-glob-"));
  return { pluginsRoot, project, global };
}

function writeSelection(path: string, body: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
}

function enabledOf(result: ReturnType<typeof resolveEnabledPluginNames>): string[] {
  if (!result.ok) throw new Error(`expected ok selection, got: ${result.reason}`);
  return [...result.enabled].toSorted();
}

test("global selection file decides when the project has none", () => {
  const { pluginsRoot, project, global } = selectionFixture();
  const globalPath = opencodeGlobalPluginSelectionPath(global);
  writeSelection(globalPath, JSON.stringify({ version: 1, enabled: ["jev"] }));
  const result = resolveEnabledPluginNames(pluginsRoot, project, global);
  expect(enabledOf(result)).toEqual(["jev"]);
  expect(result.ok && result.source).toBe("global");
  expect(result.ok && result.path).toBe(globalPath);
});

test("the project file replaces the global one, even an empty project list", () => {
  const { pluginsRoot, project, global } = selectionFixture();
  writeSelection(
    opencodeGlobalPluginSelectionPath(global),
    JSON.stringify({ version: 1, enabled: ["jev"] }),
  );
  writeSelection(
    opencodePluginSelectionPath(project),
    JSON.stringify({ version: 1, enabled: ["toolu-review"] }),
  );
  const chosen = resolveEnabledPluginNames(pluginsRoot, project, global);
  expect(enabledOf(chosen)).toEqual(["toolu-review"]);
  expect(chosen.ok && chosen.source).toBe("project");
  writeSelection(opencodePluginSelectionPath(project), JSON.stringify({ version: 1, enabled: [] }));
  const empty = resolveEnabledPluginNames(pluginsRoot, project, global);
  expect(enabledOf(empty)).toEqual([]);
  expect(empty.ok && empty.source).toBe("project");
});

test("an invalid global file is never read while a project file exists", () => {
  const { pluginsRoot, project, global } = selectionFixture();
  writeSelection(opencodeGlobalPluginSelectionPath(global), "{ not json");
  writeSelection(
    opencodePluginSelectionPath(project),
    JSON.stringify({ version: 1, enabled: ["jev"] }),
  );
  expect(enabledOf(resolveEnabledPluginNames(pluginsRoot, project, global))).toEqual(["jev"]);
});

test("an invalid explicit selection fails closed and names its path", () => {
  const cases = [
    "{ not json",
    JSON.stringify({ version: 2, enabled: ["toolu"] }),
    JSON.stringify({ version: 1, enabled: ["toolu"], extra: true }),
    JSON.stringify({ version: 1, enabled: ["toolu", 3] }),
  ];
  for (const body of cases) {
    const { pluginsRoot, project, global } = selectionFixture();
    const projectPath = opencodePluginSelectionPath(project);
    writeSelection(projectPath, body);
    const fromProject = resolveEnabledPluginNames(pluginsRoot, project, global);
    expect({ body, ok: fromProject.ok }).toEqual({ body, ok: false });
    if (!fromProject.ok) expect(fromProject.reason).toStartWith(`invalid ${projectPath}: `);

    const globalOnly = selectionFixture();
    const globalPath = opencodeGlobalPluginSelectionPath(globalOnly.global);
    writeSelection(globalPath, body);
    const fromGlobal = resolveEnabledPluginNames(
      globalOnly.pluginsRoot,
      globalOnly.project,
      globalOnly.global,
    );
    expect({ body, ok: fromGlobal.ok }).toEqual({ body, ok: false });
    if (!fromGlobal.ok) expect(fromGlobal.reason).toStartWith(`invalid ${globalPath}: `);
  }
});

test("an unreadable selection path (a directory, a dangling link) fails closed", () => {
  const { pluginsRoot, project, global } = selectionFixture();
  mkdirSync(opencodePluginSelectionPath(project), { recursive: true });
  expect(resolveEnabledPluginNames(pluginsRoot, project, global).ok).toBe(false);

  const linked = selectionFixture();
  const path = opencodePluginSelectionPath(linked.project);
  mkdirSync(dirname(path), { recursive: true });
  symlinkSync(join(linked.project, "removed-target.json"), path);
  writeSelection(
    opencodeGlobalPluginSelectionPath(linked.global),
    JSON.stringify({ version: 1, enabled: ["jev"] }),
  );
  const dangling = resolveEnabledPluginNames(linked.pluginsRoot, linked.project, linked.global);
  expect(dangling.ok).toBe(false);
  if (!dangling.ok) expect(dangling.reason).toStartWith(`invalid ${path}: `);
});

test("names that are not installed are dropped and reported, in file order", () => {
  const { pluginsRoot, project, global } = selectionFixture();
  writeSelection(
    opencodePluginSelectionPath(project),
    JSON.stringify({ version: 1, enabled: ["zeta", "jev", "alpha", "zeta"] }),
  );
  const result = resolveEnabledPluginNames(pluginsRoot, project, global);
  expect(enabledOf(result)).toEqual(["jev"]);
  expect(result.ok && result.unknown).toEqual(["zeta", "alpha"]);
});

test("no selection file enables every installed plugin; no global root reads no global file", () => {
  const { pluginsRoot, project, global } = selectionFixture();
  const all = resolveEnabledPluginNames(pluginsRoot, project, global);
  expect(enabledOf(all)).toEqual(["jev", "toolu", "toolu-review"]);
  expect(all.ok && all.source).toBe("default");
  writeSelection(
    opencodeGlobalPluginSelectionPath(global),
    JSON.stringify({ version: 1, enabled: ["jev"] }),
  );
  const withoutGlobalRoot = resolveEnabledPluginNames(pluginsRoot, project);
  expect(enabledOf(withoutGlobalRoot)).toEqual(["jev", "toolu", "toolu-review"]);
  expect(withoutGlobalRoot.ok && withoutGlobalRoot.source).toBe("default");
});
