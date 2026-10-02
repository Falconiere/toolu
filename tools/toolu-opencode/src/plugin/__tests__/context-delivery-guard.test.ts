/**
 * A prompt bundle that resolves outside its plugin directory is not spawned.
 */
import { expect, test } from "bun:test";
import { mkdir, mkdtemp, symlink, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import type { Part } from "@opencode-ai/sdk";
import type { LogLevel } from "../context.ts";
import { createContextHooks, type ContextPlan, type Spawner } from "../context-delivery.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";

type Logged = { level: LogLevel; message: string };

function plan(pluginDir: string, bundle: string): ContextPlan {
  return {
    startupLines: [],
    notices: [],
    prompt: [
      {
        plugin: "toolu",
        name: "user-prompt-submit",
        bundle,
        pluginDir,
        event: "UserPromptSubmit",
      },
    ],
    compact: [],
    bun: process.execPath,
    projectRoot: pluginDir,
    env: {},
  };
}

function textPart(text: string): Part {
  return { id: "prt_user", sessionID: "ses_a", messageID: "msg_esc", type: "text", text };
}

test("a bundle that resolves outside the plugin directory is not spawned", async () => {
  const root = await mkdtemp(join(tmpBase, "toolu-oc-escape-"));
  const pluginDir = join(root, "plugin");
  const outside = join(root, "outside.js");
  const bundle = join(pluginDir, "hooks", "dist", "user-prompt-submit.js");
  await mkdir(dirname(bundle), { recursive: true });
  await writeFile(outside, "export {}\n");
  await symlink(outside, bundle);
  let spawned = 0;
  const logged: Logged[] = [];
  const spawn: Spawner = () => {
    spawned += 1;
    return Promise.resolve({ status: "exited", exitCode: 0, stdout: "", stderr: "" });
  };
  const hooks = createContextHooks(
    plan(pluginDir, bundle),
    (level, messageText) => {
      logged.push({ level, message: messageText });
      return Promise.resolve();
    },
    spawn,
  );
  const parts: Part[] = [textPart("rename the parser")];
  await hooks.prompt(
    { sessionID: "ses_a" },
    {
      message: {
        id: "msg_esc",
        sessionID: "ses_a",
        role: "user",
        time: { created: 0 },
        agent: "build",
        model: { providerID: "probe", modelID: "scripted" },
      },
      parts,
    },
  );
  expect(spawned).toBe(0);
  expect(parts).toHaveLength(1);
  expect(logged.some((line) => line.message.includes("outside"))).toBe(true);
  await hooks.dispose();
});
