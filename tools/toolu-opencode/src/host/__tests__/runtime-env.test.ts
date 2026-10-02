/**
 * The environment toolu builds on OpenCode (#343): its own processes keep the
 * host env and HOME but lose every foreign host root; the agent's bash gets only
 * toolu's non-secret roots, one root per selected plugin, and Bun on PATH when
 * the host PATH has none.
 */
import { expect, test } from "bun:test";
import { createOpencodeClient } from "@opencode-ai/sdk";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import { bindHostContext } from "../../plugin/context.ts";
import { selectPluginsByEnabledNames } from "../../select/resolve.ts";
import {
  FOREIGN_HOST_VARS,
  pluginRootVar,
  shellEnvAdditions,
  tooluProcessEnv,
  type OpencodeRoots,
} from "../runtime-env.ts";

const REPO_ROOT = resolve(import.meta.dir, "../../../../..");
const BUN = process.execPath;

const ROOTS: OpencodeRoots = {
  projectRoot: "/work/my project",
  dataRoot: "/work/my project/.opencode/toolu/state",
  userConfigRoot: "/home/u/.config/opencode",
  repoRoot: REPO_ROOT,
  packageRoot: join(REPO_ROOT, "tools/toolu-opencode"),
};

const HOST_ENV = {
  HOME: "/home/u",
  PATH: "/usr/bin:/bin",
  EXA_API_KEY: "secret-exa",
  CLAUDE_CONFIG_DIR: "/home/u/.claude-alt",
  CLAUDE_PROJECT_DIR: "/elsewhere",
  CLAUDE_PLUGIN_ROOT: "/home/u/.claude/plugins/x",
  CLAUDE_PLUGIN_DATA: "/home/u/.claude/plugins/data/x",
  CODEX_HOME: "/home/u/.codex-alt",
  PLUGIN_ROOT: "/home/u/.codex/plugins/x",
  PLUGIN_DATA: "/home/u/.codex/data/x",
  TOOLU_PROJECT_DIR: "/elsewhere",
  TOOLU_CONFIG_DIR: "/user/override",
};

function selected(names: string[]): ReturnType<typeof selectPluginsByEnabledNames> & { ok: true } {
  const result = selectPluginsByEnabledNames(join(REPO_ROOT, "plugins"), names);
  if (!result.ok) throw new Error(result.reason);
  return result;
}

test("toolu's own processes keep HOME and the host env but no foreign host root", () => {
  const env = tooluProcessEnv(HOST_ENV, ROOTS);
  expect(env.HOME).toBe("/home/u");
  expect(env.PATH).toBe("/usr/bin:/bin");
  expect(env.EXA_API_KEY).toBe("secret-exa");
  for (const key of FOREIGN_HOST_VARS) expect(env[key]).toBeUndefined();
  expect(env).toMatchObject({
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_CONFIG_DIR: ROOTS.dataRoot,
    TOOLU_USER_CONFIG_DIR: ROOTS.userConfigRoot,
    TOOLU_PROJECT_DIR: ROOTS.projectRoot,
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    TOOLU_SETTINGS_DIR: join(REPO_ROOT, "plugins/toolu/settings"),
  });
});

test("the host's worktree stays the project root whatever TOOLU_PROJECT_DIR says", () => {
  const client = createOpencodeClient({ baseUrl: "http://127.0.0.1:9" });
  const input = { client, directory: "/work/my project/sub", worktree: "/work/my project" };
  const binding = bindHostContext(input, undefined, HOST_ENV);
  expect(binding.projectRoot).toBe("/work/my project");
});

test("bash gets toolu's roots, one root per selected plugin, and nothing copied from the host", () => {
  const { plugins } = selected(["context7", "epic-orchestrator"]);
  const names = plugins.map((plugin) => plugin.name);
  expect(names).toContain("toolu");
  expect(names).not.toContain("jira");
  const env = shellEnvAdditions({ roots: ROOTS, plugins, bun: BUN, hostPath: dirname(BUN) });
  const perPlugin = Object.fromEntries(
    names.map((name) => [pluginRootVar(name), join(REPO_ROOT, "plugins", name)]),
  );
  expect(env).toEqual({
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_CONFIG_DIR: ROOTS.dataRoot,
    TOOLU_USER_CONFIG_DIR: ROOTS.userConfigRoot,
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    TOOLU_SETTINGS_DIR: join(REPO_ROOT, "plugins/toolu/settings"),
    TOOLU_BUN: BUN,
    TOOLU_OPENCODE_ROOT: ROOTS.packageRoot,
    TOOLU_PLUGIN_ROOT: join(REPO_ROOT, "plugins/toolu"),
    ...perPlugin,
  });
  expect(env.TOOLU_PLUGIN_ROOT_CONTEXT7).toBe(join(REPO_ROOT, "plugins/context7"));
  expect(env.TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR).toBe(
    join(REPO_ROOT, "plugins/epic-orchestrator"),
  );
  expect(env.TOOLU_PLUGIN_ROOT_JIRA).toBeUndefined();
  expect(Object.values(env)).not.toContain("secret-exa");
});

test("Bun's directory is appended to PATH only when the host PATH has no bun", () => {
  const { plugins } = selected(["toolu"]);
  const empty = mkdtempSync(join(tmpdir(), "toolu-no-bun-"));
  const without = shellEnvAdditions({ roots: ROOTS, plugins, bun: BUN, hostPath: empty });
  expect(without.PATH).toBe(`${empty}${delimiter}${dirname(BUN)}`);
  const unset = shellEnvAdditions({ roots: ROOTS, plugins, bun: BUN, hostPath: undefined });
  expect(unset.PATH).toBe(dirname(BUN));
  const present = `${empty}${delimiter}${dirname(BUN)}`;
  expect(shellEnvAdditions({ roots: ROOTS, plugins, bun: BUN, hostPath: present }).PATH).toBe(
    undefined,
  );
});

test("plugin root variable names are the plugin name in upper snake case", () => {
  expect(pluginRootVar("toolu")).toBe("TOOLU_PLUGIN_ROOT_TOOLU");
  expect(pluginRootVar("epic-orchestrator")).toBe("TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR");
  expect(pluginRootVar("context7")).toBe("TOOLU_PLUGIN_ROOT_CONTEXT7");
});
