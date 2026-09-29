import { afterAll, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import {
  configRoot,
  invocation,
  pluginData,
  pluginInstallCommand,
  pluginRoot,
  projectConfigPath,
  projectDirname,
  projectRoot,
  projectStateDir,
  projectStateRoot,
} from "../host-roots.ts";

const temps: string[] = [];
afterAll(() => {
  for (const dir of temps) rmSync(dir, { recursive: true, force: true });
});

function temp(): string {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), "host-roots-")));
  temps.push(dir);
  return dir;
}

function gitRepo(): string {
  const dir = join(temp(), "repo");
  mkdirSync(join(dir, "src"), { recursive: true });
  const res = spawnSync("git", ["init", "-q", dir], { encoding: "utf8" });
  expect(res.status).toBe(0);
  return dir;
}

const HOME = "/home/u";

describe("configRoot", () => {
  test("each host has its native user root", () => {
    const env = { HOME };
    expect(configRoot({ env, host: "claude" })).toBe("/home/u/.claude");
    expect(configRoot({ env, host: "codex" })).toBe("/home/u/.codex");
    expect(configRoot({ env, host: "cursor" })).toBe("/home/u/.cursor");
    expect(configRoot({ env, host: "hermes" })).toBe("/home/u/.hermes");
    expect(configRoot({ env, host: "opencode" })).toBe("/home/u/.config/opencode");
  });

  test("host-native overrides apply to their own host only", () => {
    const env = {
      HOME,
      CLAUDE_CONFIG_DIR: "/cc",
      CODEX_HOME: "/cx",
      HERMES_HOME: "/hh",
      XDG_CONFIG_HOME: "/xdg",
    };
    expect(configRoot({ env, host: "claude" })).toBe("/cc");
    expect(configRoot({ env, host: "codex" })).toBe("/cx");
    expect(configRoot({ env, host: "hermes" })).toBe("/hh");
    expect(configRoot({ env, host: "opencode" })).toBe("/xdg/opencode");
    expect(configRoot({ env, host: "cursor" })).toBe("/home/u/.cursor");
    expect(configRoot({ env: { ...env, TOOLU_OPENCODE_HOME: "/oc" }, host: "opencode" })).toBe(
      "/oc",
    );
  });

  test("TOOLU_CONFIG_DIR wins on every host", () => {
    const env = { HOME, TOOLU_CONFIG_DIR: "/explicit", CODEX_HOME: "/wrong" };
    for (const host of ["claude", "codex", "cursor", "hermes", "opencode"] as const) {
      expect(configRoot({ env, host })).toBe("/explicit");
    }
  });

  test("detects the host from env when none is given", () => {
    expect(configRoot({ env: { HOME, PLUGIN_ROOT: "/p" } })).toBe("/home/u/.codex");
  });

  test("an unset HOME falls back to the OS home directory", () => {
    expect(configRoot({ env: {}, host: "claude" })).toBe(join(homedir(), ".claude"));
  });
});

describe("project paths", () => {
  test("TOOLU_PROJECT_DIR wins, then the host's project variable", () => {
    const env = { TOOLU_PROJECT_DIR: "/t", CLAUDE_PROJECT_DIR: "/c", CURSOR_PROJECT_DIR: "/k" };
    expect(projectRoot({ env, host: "claude" })).toBe("/t");
    const noToolu = { CLAUDE_PROJECT_DIR: "/c", CURSOR_PROJECT_DIR: "/k" };
    expect(projectRoot({ env: noToolu, host: "claude" })).toBe("/c");
    expect(projectRoot({ env: noToolu, host: "cursor" })).toBe("/k");
  });

  test("Codex ignores CLAUDE_PROJECT_DIR and falls back to the git toplevel of cwd", () => {
    const repo = gitRepo();
    const env = { CLAUDE_PROJECT_DIR: "/c" };
    expect(projectRoot({ env, host: "codex", cwd: join(repo, "src") })).toBe(repo);
  });

  test("outside a git repository there is no project root and no project paths", () => {
    const cwd = temp();
    const o = { env: {}, host: "claude" as const, cwd };
    expect(projectRoot(o)).toBeUndefined();
    expect(projectConfigPath(o)).toBeUndefined();
    expect(projectStateRoot(o)).toBeUndefined();
    expect(projectStateDir("telemetry", o)).toBeUndefined();
  });

  test("config and state paths are isolated by host dirname", () => {
    for (const host of ["claude", "codex", "cursor", "hermes", "opencode"] as const) {
      const o = { env: { TOOLU_PROJECT_DIR: "/repo" }, host };
      expect(projectDirname(o)).toBe(`.${host}`);
      expect(projectConfigPath(o)).toBe(`/repo/.${host}/toolu.config.json`);
      expect(projectStateDir("telemetry", o)).toBe(`/repo/.${host}/tmp/telemetry`);
    }
  });

  test("TOOLU_PROJECT_CONFIG_DIRNAME and an explicit root override the defaults", () => {
    const env = { TOOLU_PROJECT_DIR: "/repo", TOOLU_PROJECT_CONFIG_DIRNAME: ".test-state" };
    expect(projectStateDir("telemetry", { env, host: "codex" })).toBe(
      "/repo/.test-state/tmp/telemetry",
    );
    expect(projectStateRoot({ env, host: "codex", root: "/other" })).toBe("/other/.test-state/tmp");
  });

  test("an empty state-dir name is a caller error", () => {
    expect(() => projectStateDir("", { env: { TOOLU_PROJECT_DIR: "/r" } })).toThrow(TypeError);
  });
});

describe("plugin paths", () => {
  const env = {
    PLUGIN_ROOT: "/codex-root",
    PLUGIN_DATA: "/codex-data",
    CURSOR_PLUGIN_ROOT: "/cursor-root",
    TOOLU_PLUGIN_ROOT: "/oc-root",
    CLAUDE_PLUGIN_ROOT: "/claude-root",
    CLAUDE_PLUGIN_DATA: "/claude-data",
  };

  test("each host reads its own plugin root first, then CLAUDE_PLUGIN_ROOT", () => {
    expect(pluginRoot({ env, host: "codex" })).toBe("/codex-root");
    expect(pluginRoot({ env, host: "cursor" })).toBe("/cursor-root");
    expect(pluginRoot({ env, host: "opencode" })).toBe("/oc-root");
    expect(pluginRoot({ env, host: "claude" })).toBe("/claude-root");
    expect(pluginRoot({ env, host: "hermes" })).toBe("/claude-root");
    expect(pluginRoot({ env: { CLAUDE_PLUGIN_ROOT: "/c" }, host: "codex" })).toBe("/c");
    expect(pluginRoot({ env: {}, host: "claude" })).toBeUndefined();
  });

  test("plugin data is PLUGIN_DATA on Codex, else CLAUDE_PLUGIN_DATA", () => {
    expect(pluginData({ env, host: "codex" })).toBe("/codex-data");
    expect(pluginData({ env, host: "claude" })).toBe("/claude-data");
    expect(pluginData({ env: {}, host: "cursor" })).toBeUndefined();
  });
});

describe("invocation and install", () => {
  test("invocation syntax is host-native", () => {
    expect(invocation("toolu", "setup", { env: {}, host: "claude" })).toBe("/toolu:setup");
    expect(invocation("toolu", "setup", { env: {}, host: "codex" })).toBe("$toolu:setup");
    expect(invocation("toolu", "setup", { env: {}, host: "cursor" })).toBe("/toolu:setup");
    expect(invocation("toolu", "setup", { env: {}, host: "hermes" })).toBe("/toolu:setup");
    expect(invocation("toolu", "commit", { env: {}, host: "opencode" })).toBe("/toolu--commit");
  });

  test("an empty namespace or name is a caller error", () => {
    expect(() => invocation("", "setup", { env: {} })).toThrow(TypeError);
    expect(() => invocation("toolu", "", { env: {} })).toThrow(TypeError);
  });

  test("only Claude and Codex have a host-native per-plugin install command", () => {
    expect(pluginInstallCommand("toolu@toolu", { env: {}, host: "claude" })).toBe(
      "/plugin install toolu@toolu",
    );
    expect(pluginInstallCommand("toolu@toolu", { env: {}, host: "codex" })).toBe(
      "codex plugin add toolu@toolu",
    );
    for (const host of ["cursor", "hermes", "opencode"] as const) {
      expect(pluginInstallCommand("toolu@toolu", { env: {}, host })).toBeNull();
    }
    expect(() => pluginInstallCommand("", { env: {} })).toThrow(TypeError);
  });
});
