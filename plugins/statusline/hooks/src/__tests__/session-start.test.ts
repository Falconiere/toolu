/** statusline's SessionStart bundle through its real hooks.json launcher (ported from session-start.bats). */
import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import {
  publishedCliSuite,
  runStartupHook,
  startupEnv,
  startupRoot,
  type StartupHost,
} from "@toolu/conformance/harness/startup";
import { put } from "./harness.ts";

const PLUGIN = resolve(import.meta.dir, "../../..");
const ADVISORY =
  '{"systemMessage":"statusline: settings.json still runs the statusline through a shell, which cannot run the Bun statusline — run /statusline:setup to update it"}\n';

publishedCliSuite({
  plugin: "statusline",
  pluginRoot: PLUGIN,
  source: "hooks/dist/statusline.js",
  dir: "statusline",
  name: "statusline.sh",
  advisory:
    "statusline: bun not found on PATH — the statusline renderer needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)",
  credentials: { TYPESAFE_API_KEY: "statusline-test-key" },
  probe: { args: [], exitCode: 0, output: "ctx:0/0" },
});

/** The SessionStart stdout on `host` with Claude's settings.json holding `command`. */
async function startupWith(host: StartupHost, command: string): Promise<string> {
  using sb = createSandbox();
  const settings = { statusLine: { type: "command", command } };
  put(join(startupRoot("claude", sb), "settings.json"), JSON.stringify(settings));
  const res = await runStartupHook(PLUGIN, "session-start", sb, startupEnv(host, sb, PLUGIN));
  expect(res).toMatchObject({ exitCode: 0, stderr: "" });
  return res.stdout;
}

test.concurrent("statusline session-start: a bash statusLine on Claude gets one notice", async () => {
  expect(await startupWith("claude", "bash ~/.claude/statusline/statusline.sh")).toBe(ADVISORY);
});

test.concurrent("statusline session-start: an interpreter path counts as a shell", async () => {
  expect(await startupWith("claude", "/usr/bin/env bash ~/.claude/statusline/statusline.sh")).toBe(
    ADVISORY,
  );
});

test.concurrent("statusline session-start: a Bun statusLine on Claude is silent", async () => {
  expect(await startupWith("claude", "~/.claude/statusline/statusline.sh")).toBe("");
});

test.concurrent("statusline session-start: a custom statusLine on Claude is silent", async () => {
  expect(await startupWith("claude", "bash my-custom-bar.sh")).toBe("");
  expect(await startupWith("claude", 'bash -c "~/.claude/statusline/statusline.sh | cat"')).toBe(
    "",
  );
});

test.concurrent("statusline session-start: Codex never reads Claude's settings", async () => {
  expect(await startupWith("codex", "bash ~/.claude/statusline/statusline.sh")).toBe("");
});
