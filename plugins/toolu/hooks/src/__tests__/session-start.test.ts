import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { launcherCommand, runtimeDiagnostic } from "@toolu/core/launcher";

const PLUGIN = resolve(import.meta.dir, "../../..");
const BUNDLE = join(PLUGIN, "hooks/dist/session-start.js");

test("the committed bundle reports the Bun that runs it", () => {
  const result = spawnSync(process.execPath, [BUNDLE], { input: "{}", encoding: "utf8" });
  expect(result.status).toBe(0);
  expect(result.stderr).toBe("");
  expect(JSON.parse(result.stdout)).toEqual(runtimeDiagnostic(process.execPath, Bun.version));
});

test("hooks.json wires it through the generated launcher on startup|resume", () => {
  const command = launcherCommand({
    plugin: "toolu",
    event: "SessionStart",
    entry: "session-start",
  });
  const hooks: unknown = JSON.parse(readFileSync(join(PLUGIN, "hooks/hooks.json"), "utf8"));
  expect(hooks).toMatchObject({
    hooks: {
      SessionStart: expect.arrayContaining([
        {
          matcher: "startup|resume",
          hooks: [expect.objectContaining({ type: "command", command })],
        },
      ]),
    },
  });
});

test("the launcher runs the real bundle end to end", () => {
  const command = launcherCommand({
    plugin: "toolu",
    event: "SessionStart",
    entry: "session-start",
  });
  const result = spawnSync("sh", ["-c", command], {
    env: {
      PATH: "/usr/bin:/bin",
      HOME: PLUGIN,
      TOOLU_BUN: process.execPath,
      CLAUDE_PLUGIN_ROOT: PLUGIN,
    },
    input: "{}",
    encoding: "utf8",
  });
  expect(result.status).toBe(0);
  expect(JSON.parse(result.stdout)).toEqual(runtimeDiagnostic(process.execPath, Bun.version));
});
