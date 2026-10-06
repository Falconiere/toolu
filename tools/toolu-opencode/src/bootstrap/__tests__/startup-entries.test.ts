/** Startup entries come from each plugin's real `hooks.json` launchers (#342). */
import { expect, test } from "bun:test";
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { launcherHook } from "@toolu/core/launcher";
import { z } from "zod";
import { matcherCovers, pluginHookEntries, pluginStartupEntries } from "../entrypoint.ts";
import { PLUGINS_ROOT, copiedPlugin, fixturePlugin, tempRoot } from "./fixtures.ts";

const NATIVE_SESSION_START = readFileSync(
  resolve(
    import.meta.dir,
    "../../../../../crates/core/protocol/src/tests/fixtures/launcher-session-start.txt",
  ),
  "utf8",
);

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
  expect(entryNames("jev")).toEqual(["session-start"]);
});

test.concurrent("Jev SessionStart resolves its shipped bundle", () => {
  const plugin = join(PLUGINS_ROOT, "jev");
  expect(pluginStartupEntries(plugin)).toEqual({
    ok: true,
    entries: [{ name: "session-start", bundle: join(plugin, "hooks", "dist", "session-start.js") }],
  });
});

test.concurrent("a generated native SessionStart retains its declared shell command", () => {
  using root = tempRoot("toolu-entries-native-");
  const plugin = fixturePlugin(root.path, "toolu", {
    entries: { "session-start": "process.exit(0);\n" },
  });
  writeFileSync(
    join(plugin.pluginDir, "hooks", "hooks.json"),
    JSON.stringify({
      hooks: {
        SessionStart: [
          {
            hooks: [{ type: "command", command: NATIVE_SESSION_START, timeout: 60 }],
          },
        ],
      },
    }),
  );
  expect(pluginStartupEntries(plugin.pluginDir)).toEqual({
    ok: true,
    entries: [
      {
        name: "session-start",
        bundle: join(plugin.pluginDir, "hooks", "dist", "session-start.js"),
        command: NATIVE_SESSION_START,
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

function routing(command: string): string {
  return JSON.stringify({ hooks: { SessionStart: [{ hooks: [{ type: "command", command }] }] } });
}

function unsupported(command: string, plugin: string): string {
  return (
    `unsupported SessionStart command ${JSON.stringify(command)}; ` +
    `regenerate it with \`bun run tooling/src/check-hooks-json.ts --print ${plugin} SessionStart <entry>\``
  );
}

test.concurrent("a bundle path that cannot be stat'ed is a missing bundle, not a throw", () => {
  using root = tempRoot("toolu-entries-notdir-");
  const plugin = fixturePlugin(root.path, "blocked", { entries: { boot: "process.exit(0);\n" } });
  const dist = join(plugin.pluginDir, "hooks", "dist");
  rmSync(dist, { recursive: true });
  writeFileSync(dist, "not a directory\n");
  expect(pluginStartupEntries(plugin.pluginDir)).toEqual({
    ok: false,
    reason: "boot: missing startup bundle",
  });
});

test.concurrent("a hand-written or legacy SessionStart command is unsupported", () => {
  using root = tempRoot("toolu-entries-legacy-");
  const shell = 'bash "${CLAUDE_PLUGIN_ROOT}/hooks/register.sh"';
  const legacy = fixturePlugin(root.path, "legacy", { hooksJson: routing(shell) });
  expect(pluginStartupEntries(legacy.pluginDir)).toEqual({
    ok: false,
    reason: unsupported(shell, "legacy"),
  });
  const bare = 'bun "${CLAUDE_PLUGIN_ROOT}/hooks/dist/register.js"';
  const edited = fixturePlugin(root.path, "edited", { entries: {}, hooksJson: routing(bare) });
  expect(pluginStartupEntries(edited.pluginDir)).toEqual({
    ok: false,
    reason: unsupported(bare, "edited"),
  });
});

test.concurrent("a SessionStart hook that is not a command is a named reason", () => {
  using root = tempRoot("toolu-entries-prompt-");
  const prompt = JSON.stringify({
    hooks: { SessionStart: [{ hooks: [{ type: "prompt", prompt: "Say hi" }] }] },
  });
  const plugin = fixturePlugin(root.path, "prompter", { hooksJson: prompt });
  expect(pluginStartupEntries(plugin.pluginDir)).toEqual({
    ok: false,
    reason: 'unsupported SessionStart hook type "prompt"',
  });
});

test.concurrent("a matcher without startup is not run at plugin init", () => {
  using root = tempRoot("toolu-entries-matcher-");
  const compactOnly = fixturePlugin(root.path, "compact-only", {
    entries: { "on-compact": "process.exit(0);\n" },
    matcher: "compact",
  });
  expect(pluginStartupEntries(compactOnly.pluginDir)).toEqual({ ok: true, entries: [] });
  const compact = pluginHookEntries(compactOnly.pluginDir, "SessionStart", (matcher) =>
    matcherCovers(matcher, "compact"),
  );
  expect(compact).toEqual({
    ok: true,
    entries: [
      {
        name: "on-compact",
        bundle: join(compactOnly.pluginDir, "hooks", "dist", "on-compact.js"),
      },
    ],
  });
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
  expect(pluginStartupEntries(broken.pluginDir)).toEqual({
    ok: false,
    reason: "invalid hooks.json: SyntaxError: JSON Parse error: Expected '}'",
  });
  const noHooks = JSON.stringify({ hooks: { SessionStart: [{ matcher: "startup" }] } });
  const shapeless = fixturePlugin(root.path, "shapeless", { hooksJson: noHooks });
  expect(pluginStartupEntries(shapeless.pluginDir)).toEqual({
    ok: false,
    reason:
      "invalid hooks.json: ✖ Invalid input: expected array, received undefined\n  → at hooks.SessionStart[0].hooks",
  });
});

test.concurrent("prompt and compaction entries come from the real launchers", () => {
  const toolu = join(PLUGINS_ROOT, "toolu");
  expect(
    pluginHookEntries(toolu, "UserPromptSubmit", (matcher) => matcherCovers(matcher, "prompt")),
  ).toEqual({
    ok: true,
    entries: [
      {
        name: "user-prompt-submit",
        bundle: join(toolu, "hooks", "dist", "user-prompt-submit.js"),
      },
    ],
  });
  const compact = pluginHookEntries(toolu, "SessionStart", (matcher) =>
    matcherCovers(matcher, "compact"),
  );
  expect(compact.ok && compact.entries.map((entry) => entry.name)).toEqual(["session-start"]);
  const pre = pluginHookEntries(toolu, "PreCompact", (matcher) => matcherCovers(matcher, "auto"));
  expect(pre.ok && pre.entries.map((entry) => entry.name)).toEqual(["pre-compact"]);
  const jev = pluginHookEntries(join(PLUGINS_ROOT, "jev"), "UserPromptSubmit", (matcher) =>
    matcherCovers(matcher, "prompt"),
  );
  expect(jev.ok && jev.entries.map((entry) => entry.name)).toEqual(["user-prompt-submit"]);
  expect(
    pluginHookEntries(join(PLUGINS_ROOT, "brainstorm"), "UserPromptSubmit", (matcher) =>
      matcherCovers(matcher, "prompt"),
    ),
  ).toEqual({ ok: true, entries: [] });
});

test.concurrent("a missing prompt bundle is a reason", () => {
  using root = tempRoot("toolu-entries-prompt-missing-");
  const plugin = fixturePlugin(root.path, "prompter", {
    entries: { "user-prompt-submit": "process.exit(0);\n" },
  });
  const hook = launcherHook({
    plugin: "prompter",
    event: "UserPromptSubmit",
    entry: "user-prompt-submit",
  });
  writeFileSync(
    join(plugin.pluginDir, "hooks", "hooks.json"),
    JSON.stringify({ hooks: { UserPromptSubmit: [{ hooks: [hook] }] } }),
  );
  rmSync(join(plugin.pluginDir, "hooks", "dist", "user-prompt-submit.js"));
  expect(
    pluginHookEntries(plugin.pluginDir, "UserPromptSubmit", (matcher) =>
      matcherCovers(matcher, "prompt"),
    ),
  ).toEqual({ ok: false, reason: "user-prompt-submit: missing UserPromptSubmit bundle" });
});

test.concurrent("keys a host adds beside a launcher do not change what runs", () => {
  using root = tempRoot("toolu-entries-extra-");
  const plugin = fixturePlugin(root.path, "extra", { entries: { boot: "process.exit(0);\n" } });
  const path = join(plugin.pluginDir, "hooks", "hooks.json");
  const generated = z
    .object({
      hooks: z.object({
        SessionStart: z.array(z.looseObject({ hooks: z.array(z.looseObject({})) })),
      }),
    })
    .parse(JSON.parse(readFileSync(path, "utf8")));
  for (const group of generated.hooks.SessionStart) {
    group["description"] = "boot";
    for (const hook of group.hooks) hook["timeout"] = 30;
  }
  writeFileSync(path, JSON.stringify(generated));
  expect(pluginStartupEntries(plugin.pluginDir)).toEqual({
    ok: true,
    entries: [{ name: "boot", bundle: join(plugin.pluginDir, "hooks", "dist", "boot.js") }],
  });
});
