/**
 * toolu's UserPromptSubmit bundle through its real hooks.json launcher (#263).
 * Parity with the bash hook lives in lifecycle-golden.test.ts; this file
 * covers the wiring a host exercises.
 */
import { expect, test } from "bun:test";
import { join } from "node:path";
import { launcherHook, missingRuntimeMessage } from "@toolu/core/launcher";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { hookCommand } from "@toolu/conformance/harness/startup";
import { PLUGIN, launchCount } from "./lifecycle-cases.ts";

const HOOKS = Bun.file(join(PLUGIN, "hooks", "hooks.json"));

test("hooks.json routes every prompt to the generated launcher", async () => {
  const hooks: unknown = await HOOKS.json();
  expect(hooks).toMatchObject({
    hooks: {
      UserPromptSubmit: expect.arrayContaining([
        {
          hooks: [
            launcherHook({
              plugin: "toolu",
              event: "UserPromptSubmit",
              entry: "user-prompt-submit",
            }),
          ],
        },
      ]),
    },
  });
  expect(await launchCount("UserPromptSubmit", "user-prompt-submit")).toBe(1);
});

test("the launcher runs the bundle with the prompt on stdin", async () => {
  using sb = createSandbox({ git: true });
  const res = await run(
    ["sh", "-c", hookCommand(PLUGIN, "UserPromptSubmit", "user-prompt-submit")],
    {
      cwd: sb.project,
      env: {
        PATH: "/usr/bin:/bin",
        HOME: sb.home,
        TOOLU_BUN: process.execPath,
        CLAUDE_PLUGIN_ROOT: PLUGIN,
      },
      stdin: JSON.stringify({
        hook_event_name: "UserPromptSubmit",
        prompt: "add coverage for the parser",
      }),
    },
  );
  expect(res).toMatchObject({ exitCode: 0, stderr: "" });
  expect(JSON.parse(res.stdout)).toEqual({
    hookSpecificOutput: {
      hookEventName: "UserPromptSubmit",
      additionalContext: "Tests: real-world data only, NO mocks.",
    },
  });
});

test("without Bun the prompt still goes through with an advisory", async () => {
  using sb = createSandbox({ git: true });
  const res = await run(
    ["sh", "-c", hookCommand(PLUGIN, "UserPromptSubmit", "user-prompt-submit")],
    {
      cwd: sb.project,
      env: { PATH: "/usr/bin:/bin", HOME: sb.home, CLAUDE_PLUGIN_ROOT: PLUGIN },
      stdin: JSON.stringify({ prompt: "fix the bug" }),
    },
  );
  expect(res.exitCode).toBe(0);
  expect(JSON.parse(res.stdout)).toEqual({ systemMessage: missingRuntimeMessage("toolu") });
});
