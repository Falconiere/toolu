import { expect, test } from "bun:test";
import {
  existsSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { bootstrapRuntime } from "../runtime.ts";
import { bootstrapCommand, pluginBootstrapScript } from "../entrypoint.ts";
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
  const postA = resultA.artifacts.find((a) => /\/post-tools\.d\/[^/]+\.js$/.test(a));
  const postB = resultB.artifacts.find((a) => /\/post-tools\.d\/[^/]+\.js$/.test(a));
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
    expect(bothResult.artifacts.some((a) => /\/post-tools\.d\/[^/]+\.js$/.test(a))).toBe(true);
  }
});

test("AC-1b: core-only bootstrap is ready even when the gate notice is pinned", async () => {
  // Pinning delivery skips the conditional .gate-preset-notice-v6 write. A
  // core-only selection (no register.sh, so no registry modules) used to then
  // produce zero artifacts and fail closed. The deterministic
  // .session-start-ready marker must carry readiness on its own.
  const root = repoRoot();
  const pluginsRoot = join(root, "plugins");
  const project = mkdtempSync(join(tmpBase, "toolu-bs-pinned-"));
  mkdirSync(join(project, ".opencode", "toolu"), { recursive: true });
  writeFileSync(
    opencodePluginSelectionPath(project),
    JSON.stringify({ version: 1, enabled: ["toolu"] }),
  );
  writeFileSync(
    join(project, ".opencode", "toolu.config.json"),
    JSON.stringify({ version: 1, gates: { preset: "strict" } }),
  );
  const select = selectPluginsWithDependencies(pluginsRoot, project);
  expect(select.ok).toBe(true);
  if (!select.ok) {
    return;
  }
  const dataRoot = mkdtempSync(join(tmpBase, "toolu-bs-pinned-data-"));
  const result = await bootstrapRuntime({
    repoRoot: root,
    projectRoot: project,
    dataRoot,
    plugins: select.plugins,
    isolatedHome: isolatedHome(),
  });
  expect(result.status).toBe("ready");
  if (result.status === "ready") {
    expect(result.artifacts.some((a) => a.endsWith(".session-start-ready"))).toBe(true);
    expect(result.artifacts.some((a) => a.endsWith(".gate-preset-notice-v6"))).toBe(false);
  }
});

test("#269: a plugin with only a TypeScript startup bundle bootstraps through its launcher", () => {
  const context7 = join(repoRoot(), "plugins", "context7");
  const script = pluginBootstrapScript(context7);
  expect(script).toBe(join(context7, "hooks", "dist", "session-start.js"));
  const command = bootstrapCommand(script ?? "", "context7", context7);
  expect(command.argv.slice(0, 2)).toEqual(["sh", "-c"]);
  expect(command.argv[2]).toContain('"${CLAUDE_PLUGIN_ROOT}/hooks/dist/session-start.js"');
  expect(command.env).toEqual({ CLAUDE_PLUGIN_ROOT: context7 });
  expect(bootstrapCommand("/p/hooks/register.sh", "p", "/p")).toEqual({
    argv: ["bash", "/p/hooks/register.sh"],
    env: {},
  });
  expect(pluginBootstrapScript(mkdtempSync(join(tmpBase, "toolu-bs-empty-")))).toBeNull();
});

test("#268: a plugin with a TypeScript register bundle bootstraps through its register launcher", () => {
  const astGrep = join(repoRoot(), "plugins", "ast-grep");
  const script = pluginBootstrapScript(astGrep);
  expect(script).toBe(join(astGrep, "hooks", "dist", "register.js"));
  const command = bootstrapCommand(script ?? "", "ast-grep", astGrep);
  expect(command.argv[2]).toContain('"${CLAUDE_PLUGIN_ROOT}/hooks/dist/register.js"');
  expect(command.env).toEqual({ CLAUDE_PLUGIN_ROOT: astGrep });
});

test("#268: bootstrapping ast-grep registers its bundled modules under the OpenCode data root", async () => {
  const root = repoRoot();
  const pluginsRoot = join(root, "plugins");
  const project = mkdtempSync(join(tmpBase, "toolu-bs-ag-"));
  mkdirSync(join(project, ".opencode", "toolu"), { recursive: true });
  writeFileSync(
    opencodePluginSelectionPath(project),
    JSON.stringify({ version: 1, enabled: ["toolu", "ast-grep"] }),
  );
  const select = selectPluginsWithDependencies(pluginsRoot, project);
  expect(select.ok).toBe(true);
  if (!select.ok) {
    return;
  }
  const dataRoot = join(project, ".opencode", "toolu", "state");
  const result = await bootstrapRuntime({
    repoRoot: root,
    projectRoot: project,
    dataRoot,
    plugins: select.plugins,
    isolatedHome: isolatedHome(),
  });
  expect(result.status).toBe("ready");
  if (result.status !== "ready") {
    return;
  }
  const modules = result.artifacts.map((a) =>
    a.slice(a.lastIndexOf("/", a.lastIndexOf("/") - 1) + 1),
  );
  expect(modules).toContain("pre-tools.d/ast-grep@toolu__search-nudge.js");
  expect(modules).toContain("post-tools.d/ast-grep@toolu__byte-savings.js");
});

test("#265: a plugin with a TypeScript register bundle bootstraps through its register launcher", () => {
  const tsQuality = join(repoRoot(), "plugins", "ts-quality");
  const script = pluginBootstrapScript(tsQuality);
  expect(script).toBe(join(tsQuality, "hooks", "dist", "register.js"));
  const command = bootstrapCommand(script ?? "", "ts-quality", tsQuality);
  expect(command.argv[2]).toContain('"${CLAUDE_PLUGIN_ROOT}/hooks/dist/register.js"');
  expect(command.env).toEqual({ CLAUDE_PLUGIN_ROOT: tsQuality });
});

test("#269: bootstrapping context7 publishes its search CLI under the OpenCode data root", async () => {
  const root = repoRoot();
  const pluginsRoot = join(root, "plugins");
  const project = mkdtempSync(join(tmpBase, "toolu-bs-c7-"));
  mkdirSync(join(project, ".opencode", "toolu"), { recursive: true });
  writeFileSync(
    opencodePluginSelectionPath(project),
    JSON.stringify({ version: 1, enabled: ["toolu", "context7"] }),
  );
  const select = selectPluginsWithDependencies(pluginsRoot, project);
  expect(select.ok).toBe(true);
  if (!select.ok) {
    return;
  }
  const dataRoot = join(project, ".opencode", "toolu", "state");
  const result = await bootstrapRuntime({
    repoRoot: root,
    projectRoot: project,
    dataRoot,
    plugins: select.plugins,
    isolatedHome: isolatedHome(),
  });
  expect(result).toMatchObject({ status: "ready" });
  expect(readlinkSync(join(dataRoot, "context7", "search.sh"))).toBe(
    join(pluginsRoot, "context7", "hooks", "dist", "search.js"),
  );
});
