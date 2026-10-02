/**
 * Startup keeps the user's HOME and never resolves a Claude or Codex home
 * (#343). The whole catalog starts with a HOME that holds poisoned `.claude`
 * and `.codex` trees and with every foreign host root pointing at more poison;
 * a fixture entry reports the environment it was given. Nothing outside the
 * data root changes, and the poisoned config is never applied.
 */
import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { lstatSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { FOREIGN_HOST_VARS } from "../../host/runtime-env.ts";
import { listPluginManifests } from "../../inventory/scan.ts";
import { selectPluginsByEnabledNames } from "../../select/resolve.ts";
import { bootstrapRuntime } from "../runtime.ts";
import { fixturePlugin, PLUGINS_ROOT, REPO_ROOT, tempRoot } from "./fixtures.ts";

/** Prints the variables startup cares about as SessionStart context. */
const ENV_PROBE = `const keys = ${JSON.stringify([
  "HOME",
  "TOOLU_CONFIG_DIR",
  "TOOLU_USER_CONFIG_DIR",
  "TOOLU_PROJECT_DIR",
  "TOOLU_PROJECT_CONFIG_DIRNAME",
  "TOOLU_HOST_OVERRIDE",
  "TOOLU_SETTINGS_DIR",
  "TOOLU_PLUGIN_ROOT",
  "CLAUDE_PLUGIN_ROOT",
  ...FOREIGN_HOST_VARS,
])};
const seen = Object.fromEntries(keys.map((key) => [key, process.env[key] ?? null]));
process.stdout.write(JSON.stringify({ hookSpecificOutput: { hookEventName: "SessionStart", additionalContext: JSON.stringify(seen) } }));
`;

/** Disables toolu's session context: if it were read, toolu's startup context would vanish. */
const POISON_CONFIG = JSON.stringify({ version: 1, hooks: { "session-start": false } });

/** Every path under `dir` with its bytes hash (files) and mtime, depth first. */
function snapshot(dir: string, prefix = "", out: string[] = []): string[] {
  const entries = readdirSync(join(dir, prefix), { withFileTypes: true }).toSorted((a, b) =>
    a.name.localeCompare(b.name),
  );
  for (const entry of entries) {
    const rel = join(prefix, entry.name);
    const stat = lstatSync(join(dir, rel));
    if (entry.isDirectory()) {
      out.push(`${rel}/ ${stat.mtimeMs}`);
      snapshot(dir, rel, out);
    } else {
      const hash = createHash("sha256")
        .update(readFileSync(join(dir, rel)))
        .digest("hex");
      out.push(`${rel} ${hash} ${stat.mtimeMs}`);
    }
  }
  return out;
}

function poisonedTree(dir: string): string {
  mkdirSync(join(dir, "settings"), { recursive: true });
  writeFileSync(join(dir, "settings", "protected-files.txt"), "nothing-is-protected\n");
  writeFileSync(join(dir, "toolu.config.json"), POISON_CONFIG);
  return dir;
}

test("the catalog starts with the user's HOME and no foreign host root", async () => {
  using root = tempRoot("toolu-startup-env-");
  const home = join(root.path, "home");
  poisonedTree(join(home, ".claude"));
  poisonedTree(join(home, ".codex"));
  const poisoned = ["claude-alt", "codex-alt", "plugin-root", "plugin-data"].map((name) =>
    poisonedTree(join(root.path, name)),
  );
  const [claudeAlt = "", codexAlt = "", pluginRoot = "", pluginData = ""] = poisoned;
  const project = join(root.path, "project");
  mkdirSync(project);
  const xdg = join(root.path, "xdg");
  const probe = fixturePlugin(root.path, "env-probe", { entries: { "session-start": ENV_PROBE } });
  const catalog = (listPluginManifests(PLUGINS_ROOT) ?? []).map((plugin) => plugin.name);
  const selected = selectPluginsByEnabledNames(PLUGINS_ROOT, catalog);
  if (!selected.ok) throw new Error(selected.reason);
  const before = [home, ...poisoned].map((dir) => snapshot(dir));
  const data = join(root.path, "data");

  const result = await bootstrapRuntime({
    repoRoot: REPO_ROOT,
    projectRoot: project,
    dataRoot: data,
    plugins: [...selected.plugins, probe],
    env: {
      HOME: home,
      XDG_CONFIG_HOME: xdg,
      TOOLU_BUN: process.execPath,
      CLAUDE_CONFIG_DIR: claudeAlt,
      CODEX_HOME: codexAlt,
      PLUGIN_ROOT: pluginRoot,
      CLAUDE_PLUGIN_DATA: pluginData,
      CLAUDE_PROJECT_DIR: claudeAlt,
    },
  });

  if (result.status !== "ready") throw new Error(result.reason);
  const entry = result.plugins.find((plugin) => plugin.plugin === "env-probe")?.entries[0];
  const seen: unknown = JSON.parse(entry?.additionalContext ?? "{}");
  expect(seen).toEqual({
    HOME: home,
    TOOLU_CONFIG_DIR: data,
    TOOLU_USER_CONFIG_DIR: join(xdg, "opencode"),
    TOOLU_PROJECT_DIR: project,
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_SETTINGS_DIR: join(REPO_ROOT, "plugins/toolu/settings"),
    TOOLU_PLUGIN_ROOT: probe.pluginDir,
    CLAUDE_PLUGIN_ROOT: probe.pluginDir,
    ...Object.fromEntries(
      FOREIGN_HOST_VARS.filter((key) => key !== "CLAUDE_PLUGIN_ROOT").map((key) => [key, null]),
    ),
  });
  const toolu = result.plugins.find((plugin) => plugin.plugin === "toolu")?.entries[0];
  expect(toolu?.additionalContext ?? "").toContain("Session Protocol");
  expect([home, ...poisoned].map((dir) => snapshot(dir))).toEqual(before);
}, 240_000);
