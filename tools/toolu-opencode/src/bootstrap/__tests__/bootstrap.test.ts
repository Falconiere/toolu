import { expect, test } from "bun:test";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { bootstrapRuntime } from "../runtime.ts";
import { bootstrapWithNoOpRunner } from "../test-helpers.ts";
import { listPluginManifests } from "../../inventory/scan.ts";
import { selectPluginsWithDependencies } from "../../select/resolve.ts";
import { opencodePluginSelectionPath } from "../../host/roots.ts";
const tmpBase = process.env.TMPDIR ?? "/tmp";

function repoRoot(): string {
  return join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
}

function isolatedHome(): string {
  return mkdtempSync(join(tmpBase, "toolu-oc-home-"));
}

test("AC-4: exit 0 without output artifacts is NotReady", async () => {
  const project = mkdtempSync(join(tmpBase, "toolu-bs-nr-"));
  const dataRoot = mkdtempSync(join(tmpBase, "toolu-bs-nr-data-"));
  const pluginsRoot = join(repoRoot(), "plugins");
  const manifests = listPluginManifests(pluginsRoot);
  const toolu = manifests?.find((m) => m.name === "toolu");
  expect(toolu).toBeDefined();
  if (!toolu) {
    return;
  }
  const result = await bootstrapWithNoOpRunner({
    repoRoot: repoRoot(),
    projectRoot: project,
    dataRoot,
    plugins: [toolu],
    isolatedHome: isolatedHome(),
  });
  expect(result.status).toBe("not-ready");
});

test("AC-3: two project roots do not share bootstrap state", async () => {
  const root = repoRoot();
  const pluginsRoot = join(root, "plugins");
  const projectA = mkdtempSync(join(tmpBase, "toolu-bs-a-"));
  const projectB = mkdtempSync(join(tmpBase, "toolu-bs-b-"));
  mkdirSync(join(projectA, ".opencode", "toolu"), { recursive: true });
  mkdirSync(join(projectB, ".opencode", "toolu"), { recursive: true });
  writeFileSync(
    opencodePluginSelectionPath(projectA),
    JSON.stringify({ version: 1, enabled: ["ts-quality"] }),
  );
  writeFileSync(
    opencodePluginSelectionPath(projectB),
    JSON.stringify({ version: 1, enabled: ["ts-quality"] }),
  );
  const selectA = selectPluginsWithDependencies(pluginsRoot, projectA);
  const selectB = selectPluginsWithDependencies(pluginsRoot, projectB);
  expect(selectA.ok && selectB.ok).toBe(true);
  if (!selectA.ok || !selectB.ok) {
    return;
  }

  const dataA = join(projectA, ".opencode", "toolu", "state");
  const dataB = join(projectB, ".opencode", "toolu", "state");
  const homeA = isolatedHome();
  const homeB = isolatedHome();

  const resultA = await bootstrapRuntime({
    repoRoot: root,
    projectRoot: projectA,
    dataRoot: dataA,
    plugins: selectA.plugins,
    isolatedHome: homeA,
  });
  const resultB = await bootstrapRuntime({
    repoRoot: root,
    projectRoot: projectB,
    dataRoot: dataB,
    plugins: selectB.plugins,
    isolatedHome: homeB,
  });
  expect(resultA.status).toBe("ready");
  expect(resultB.status).toBe("ready");
  if (resultA.status !== "ready" || resultB.status !== "ready") {
    return;
  }
  const postA = resultA.artifacts.find((a) => /\/post-tools\.d\/[^/]+\.sh$/.test(a));
  const postB = resultB.artifacts.find((a) => /\/post-tools\.d\/[^/]+\.sh$/.test(a));
  expect(postA).toBeDefined();
  expect(postB).toBeDefined();
  if (!postA || !postB) {
    return;
  }
  expect(postA.startsWith(dataA + "/")).toBe(true);
  expect(postB.startsWith(dataB + "/")).toBe(true);
  expect(postA).not.toBe(postB);
  expect(readFileSync(postA, "utf8").length).toBeGreaterThan(0);
  expect(existsSync(join(dataA, "toolu"))).toBe(true);
  expect(existsSync(join(dataB, "toolu"))).toBe(true);
});

test("AC-1: core-only session-start and core+quality register bootstrap", async () => {
  const root = repoRoot();
  const pluginsRoot = join(root, "plugins");
  const projectCore = mkdtempSync(join(tmpBase, "toolu-bs-core-"));
  mkdirSync(join(projectCore, ".opencode", "toolu"), { recursive: true });
  writeFileSync(
    opencodePluginSelectionPath(projectCore),
    JSON.stringify({ version: 1, enabled: ["toolu"] }),
  );
  const coreSelect = selectPluginsWithDependencies(pluginsRoot, projectCore);
  expect(coreSelect.ok).toBe(true);
  if (!coreSelect.ok) {
    return;
  }
  const coreData = mkdtempSync(join(tmpBase, "toolu-bs-core-data-"));
  const coreResult = await bootstrapRuntime({
    repoRoot: root,
    projectRoot: projectCore,
    dataRoot: coreData,
    plugins: coreSelect.plugins,
    isolatedHome: isolatedHome(),
  });
  expect(coreResult.status).toBe("ready");

  const projectBoth = mkdtempSync(join(tmpBase, "toolu-bs-both-"));
  mkdirSync(join(projectBoth, ".opencode", "toolu"), { recursive: true });
  writeFileSync(
    opencodePluginSelectionPath(projectBoth),
    JSON.stringify({ version: 1, enabled: ["toolu", "ts-quality"] }),
  );
  const bothSelect = selectPluginsWithDependencies(pluginsRoot, projectBoth);
  expect(bothSelect.ok).toBe(true);
  if (!bothSelect.ok) {
    return;
  }
  const bothData = mkdtempSync(join(tmpBase, "toolu-bs-both-data-"));
  const bothResult = await bootstrapRuntime({
    repoRoot: root,
    projectRoot: projectBoth,
    dataRoot: bothData,
    plugins: bothSelect.plugins,
    isolatedHome: isolatedHome(),
  });
  expect(bothResult.status).toBe("ready");
  if (bothResult.status === "ready") {
    expect(bothResult.artifacts.some((a) => /\/post-tools\.d\/[^/]+\.sh$/.test(a))).toBe(true);
  }
});
