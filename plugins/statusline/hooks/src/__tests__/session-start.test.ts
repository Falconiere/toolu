/** statusline's SessionStart bundle through its real hooks.json launcher (ported from session-start.bats). */
import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { bundlePath } from "@toolu/conformance/harness/entry-command";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import {
  publishedCliSuite,
  runStartupHook,
  startupEnv,
  startupRoot,
  type StartupHost,
} from "@toolu/conformance/harness/startup";
import { put } from "./harness.ts";

const PLUGIN = resolve(import.meta.dir, "../../..");
const StartupSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("startup"),
  host: z.enum(["claude", "codex"]),
  commands: z.array(z.string()).min(1),
  expected: z.string(),
});
const cases = readCaseFile(
  resolve(import.meta.dir, "../../../../../fixtures/statusline/cases.json"),
)
  .filter((raw) => raw.kind === "startup")
  .map((raw) => StartupSchema.parse(raw));

publishedCliSuite({
  plugin: "statusline",
  pluginRoot: PLUGIN,
  source: bundlePath("", "statusline"),
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

for (const c of cases) {
  test.concurrent(c.name, async () => {
    for (const command of c.commands) expect(await startupWith(c.host, command)).toBe(c.expected);
  });
}
