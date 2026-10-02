/** Startup entries come from each plugin's real `hooks.json` launchers (#342). */
import { expect, test } from "bun:test";
import { rmSync } from "node:fs";
import { join } from "node:path";
import { pluginStartupEntries } from "../entrypoint.ts";
import { PLUGINS_ROOT, copiedPlugin, fixturePlugin, tempRoot } from "./fixtures.ts";

function entryNames(plugin: string): string[] {
  const plan = pluginStartupEntries(join(PLUGINS_ROOT, plugin));
  if (!plan.ok) throw new Error(plan.reason);
  return plan.entries.map((entry) => entry.name);
}

test.concurrent("every catalog SessionStart launcher is an entry, in file order", () => {
  expect(entryNames("ts-quality")).toEqual(["register", "check-toolu"]);
  expect(entryNames("python-quality")).toEqual(["register", "check-toolu"]);
  expect(entryNames("rust-quality")).toEqual(["register", "check-toolu"]);
  expect(entryNames("ast-grep")).toEqual(["register"]);
  expect(entryNames("toolu")).toEqual(["session-start"]);
  expect(entryNames("epic-orchestrator")).toEqual(["check-deps"]);
  expect(entryNames("brainstorm")).toEqual([]);
  const plan = pluginStartupEntries(join(PLUGINS_ROOT, "context7"));
  expect(plan).toEqual({
    ok: true,
    entries: [
      {
        name: "session-start",
        bundle: join(PLUGINS_ROOT, "context7", "hooks", "dist", "session-start.js"),
      },
    ],
  });
});

test.concurrent("a deleted declared bundle fails the plan before anything runs", () => {
  using root = tempRoot("toolu-entries-missing-");
  const plugin = copiedPlugin(root.path, "ts-quality");
  rmSync(join(plugin.pluginDir, "hooks", "dist", "check-toolu.js"));
  expect(pluginStartupEntries(plugin.pluginDir)).toEqual({
    ok: false,
    reason: "check-toolu: missing startup bundle",
  });
});

test.concurrent("a hand-written or legacy SessionStart command is unsupported", () => {
  using root = tempRoot("toolu-entries-legacy-");
  const hooksJson = JSON.stringify({
    hooks: {
      SessionStart: [
        {
          hooks: [{ type: "command", command: 'bash "${CLAUDE_PLUGIN_ROOT}/hooks/register.sh"' }],
        },
      ],
    },
  });
  const legacy = fixturePlugin(root.path, "legacy", { hooksJson });
  expect(pluginStartupEntries(legacy.pluginDir)).toEqual({
    ok: false,
    reason: "unsupported SessionStart command",
  });
  const edited = JSON.stringify({
    hooks: {
      SessionStart: [
        {
          hooks: [
            { type: "command", command: 'bun "${CLAUDE_PLUGIN_ROOT}/hooks/dist/register.js"' },
          ],
        },
      ],
    },
  });
  const handEdited = fixturePlugin(root.path, "edited", { entries: {}, hooksJson: edited });
  expect(pluginStartupEntries(handEdited.pluginDir)).toMatchObject({ ok: false });
});

test.concurrent("a matcher without startup is not run at plugin init", () => {
  using root = tempRoot("toolu-entries-matcher-");
  const compactOnly = fixturePlugin(root.path, "compact-only", {
    entries: { "on-compact": "process.exit(0);\n" },
    matcher: "compact",
  });
  expect(pluginStartupEntries(compactOnly.pluginDir)).toEqual({ ok: true, entries: [] });
  const any = fixturePlugin(root.path, "any-source", {
    entries: { boot: "process.exit(0);\n" },
    matcher: "*",
  });
  expect(pluginStartupEntries(any.pluginDir)).toMatchObject({
    ok: true,
    entries: [{ name: "boot" }],
  });
});

test.concurrent("an invalid hooks.json is a reason, not an empty startup", () => {
  using root = tempRoot("toolu-entries-invalid-");
  const broken = fixturePlugin(root.path, "broken", { hooksJson: "{ not json" });
  const plan = pluginStartupEntries(broken.pluginDir);
  expect(plan.ok).toBe(false);
  if (!plan.ok) expect(plan.reason).toStartWith("invalid hooks.json");
  const extra = JSON.stringify({
    hooks: { SessionStart: [{ hooks: [], unexpected: true }] },
  });
  const strict = fixturePlugin(root.path, "strict", { hooksJson: extra });
  expect(pluginStartupEntries(strict.pluginDir)).toMatchObject({ ok: false });
});
