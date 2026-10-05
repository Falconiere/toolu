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
  applyShellEnv,
  DATA_ROOT_MARKER,
  FOREIGN_HOST_VARS,
  pluginRootVar,
  shellEnvFor,
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
  TYPESAFE_API_KEY: "secret-jev",
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
  expect(env.TYPESAFE_API_KEY).toBe("secret-jev");
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
  const { plugins } = selected(["jev", "epic-orchestrator"]);
  const names = plugins.map((plugin) => plugin.name);
  expect(names).toContain("toolu");
  const shell = shellEnvFor({ roots: ROOTS, plugins, bun: BUN, host: HOST_ENV });
  const perPlugin = Object.fromEntries(
    names.map((name) => [pluginRootVar(name), join(REPO_ROOT, "plugins", name)]),
  );
  expect(shell).toEqual({
    vars: {
      TOOLU_HOST_OVERRIDE: "opencode",
      TOOLU_CONFIG_DIR: ROOTS.dataRoot,
      [DATA_ROOT_MARKER]: ROOTS.dataRoot,
      TOOLU_USER_CONFIG_DIR: ROOTS.userConfigRoot,
      TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
      TOOLU_SETTINGS_DIR: join(REPO_ROOT, "plugins/toolu/settings"),
      TOOLU_PROJECT_DIR: "",
      TOOLU_BUN: BUN,
      TOOLU_OPENCODE_ROOT: ROOTS.packageRoot,
      TOOLU_PLUGIN_ROOT: join(REPO_ROOT, "plugins/toolu"),
      ...perPlugin,
    },
    bun: BUN,
    hostPath: "/usr/bin:/bin",
  });
  expect(shell.vars.TOOLU_PLUGIN_ROOT_JEV).toBe(join(REPO_ROOT, "plugins/jev"));
  expect(shell.vars.TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR).toBe(
    join(REPO_ROOT, "plugins/epic-orchestrator"),
  );
  expect(Object.values(shell.vars)).not.toContain("secret-jev");
});

test("TOOLU_PROJECT_DIR is blanked in bash only when the host exports one", () => {
  const { plugins } = selected(["toolu"]);
  const exported = shellEnvFor({ roots: ROOTS, plugins, bun: BUN, host: HOST_ENV });
  expect(exported.vars.TOOLU_PROJECT_DIR).toBe("");
  const quiet = shellEnvFor({ roots: ROOTS, plugins, bun: BUN, host: { PATH: "/usr/bin" } });
  expect("TOOLU_PROJECT_DIR" in quiet.vars).toBe(false);
});

test("Bun's directory is appended to the PATH bash will see, only when it has no bun", () => {
  const { plugins } = selected(["toolu"]);
  const empty = mkdtempSync(join(tmpdir(), "toolu-no-bun-"));
  const withBun = `${empty}${delimiter}${dirname(BUN)}`;
  const apply = (hostPath: string | undefined, env: Record<string, string>) => {
    const host: Record<string, string> = hostPath === undefined ? {} : { PATH: hostPath };
    applyShellEnv(shellEnvFor({ roots: ROOTS, plugins, bun: BUN, host }), env);
    return env;
  };
  expect(apply(empty, { KEEP: "1" })).toMatchObject({
    KEEP: "1",
    PATH: `${empty}${delimiter}${dirname(BUN)}`,
  });
  expect(apply(undefined, {}).PATH).toBe(dirname(BUN));
  expect(apply(withBun, {}).PATH).toBeUndefined();
  // An earlier plugin's PATH is kept and extended, never replaced by the host's.
  expect(apply(withBun, { PATH: `${empty}${delimiter}/opt/x` }).PATH).toBe(
    `${empty}${delimiter}/opt/x${delimiter}${dirname(BUN)}`,
  );
  expect(apply(empty, { PATH: withBun }).PATH).toBe(withBun);
});

test("plugin root variable names are the plugin name in upper snake case", () => {
  expect(pluginRootVar("toolu")).toBe("TOOLU_PLUGIN_ROOT_TOOLU");
  expect(pluginRootVar("epic-orchestrator")).toBe("TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR");
  expect(pluginRootVar("jev")).toBe("TOOLU_PLUGIN_ROOT_JEV");
});
