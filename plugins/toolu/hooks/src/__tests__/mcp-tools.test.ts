/**
 * AC-4, AC-9 (#260): the `mcp__` command from hooks.json, spawned the way
 * Claude Code and Codex spawn it, runs the committed mcp-tools bundle: a
 * blocklisted server asks on Claude and is denied on Codex, anything else is
 * silent, and the bundle does not carry the shell parser.
 */
import { expect, test } from "bun:test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { bundlePath, launchedArgv } from "@toolu/conformance/harness/entry-command";
import { mcpFixture, toStdin } from "@toolu/conformance/harness/fixtures";
import { pretoolEnv, type PretoolHost } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { z } from "zod";

const PLUGIN = resolve(import.meta.dir, "../../..");
const HooksSchema = z.object({
  hooks: z.object({
    PreToolUse: z.array(
      z.object({ matcher: z.string(), hooks: z.array(z.object({ command: z.string() })) }),
    ),
  }),
});

function mcpCommand(): string {
  const hooks = HooksSchema.parse(
    JSON.parse(readFileSync(join(PLUGIN, "hooks/hooks.json"), "utf8")),
  );
  const entry = hooks.hooks.PreToolUse.find((e) => e.matcher === "mcp__");
  const command = entry?.hooks[0]?.command;
  if (command === undefined) throw new Error("hooks.json has no mcp__ command");
  return command;
}

function settings(sb: Sandbox): string {
  const dir = join(sb.root, "settings");
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "mcp-blocklist.txt"), "exampleblocked -> use the CLI\n");
  return dir;
}

async function hook(
  host: PretoolHost,
  server: string | undefined,
  stdin?: string,
  config?: string,
) {
  using sb = createSandbox({ git: true });
  if (config !== undefined) {
    mkdirSync(sb.configDir(host, "project"), { recursive: true });
    writeFileSync(join(sb.configDir(host, "project"), "toolu.config.json"), config);
  }
  const payload =
    stdin ??
    JSON.stringify(toStdin(host, mcpFixture(server ?? "x", "search", {}), { cwd: sb.project }));
  const env = pretoolEnv(sb, host, { TOOLU_SETTINGS_DIR: settings(sb) });
  return await run(launchedArgv({ plugin: "toolu", event: "PreToolUse", entry: "mcp-tools" }, PLUGIN), {
    cwd: sb.project,
    env,
    stdin: payload,
  });
}

const Decision = z.object({
  hookSpecificOutput: z.object({
    permissionDecision: z.string(),
    permissionDecisionReason: z.string(),
  }),
});

test.concurrent("a blocklisted server asks on Claude", async () => {
  const out = await hook("claude", "exampleblocked");
  expect({ exitCode: out.exitCode, stderr: out.stderr }).toEqual({ exitCode: 0, stderr: "" });
  const decision = Decision.parse(JSON.parse(out.stdout)).hookSpecificOutput;
  expect(decision.permissionDecision).toBe("ask");
  expect(decision.permissionDecisionReason).toContain("Use instead: use the CLI");
});

test.concurrent("a blocklisted server is denied on Codex", async () => {
  const out = await hook("codex", "exampleblocked");
  expect({ exitCode: out.exitCode, stderr: out.stderr }).toEqual({ exitCode: 0, stderr: "" });
  expect(Decision.parse(JSON.parse(out.stdout)).hookSpecificOutput.permissionDecision).toBe("deny");
});

for (const [name, server, stdin] of [
  ["an unlisted server", "other", undefined],
  ["empty stdin", undefined, ""],
  ["non-JSON stdin", undefined, "not json\n"],
  ["a JSON array", undefined, "[]"],
] as const) {
  test.concurrent(`${name} is silent`, async () => {
    const out = await hook("claude", server, stdin);
    expect({ stdout: out.stdout, exitCode: out.exitCode }).toEqual({ stdout: "", exitCode: 0 });
  });
}

test("the bundle does not inline the shell parser", () => {
  expect(mcpCommand()).toContain("mcp-tools");
  const bundle = readFileSync(bundlePath(PLUGIN, "mcp-tools"), "utf8");
  expect(bundle).not.toContain("analyzeShell");
  expect(bundle).not.toContain("unbash");
});

test.concurrent("a malformed config is reported on stderr and the listed server still asks", async () => {
  const out = await hook("claude", "exampleblocked", undefined, "{not json");
  expect(out.exitCode).toBe(0);
  expect(out.stderr).toMatch(/^toolu-config: malformed JSON in .*toolu\.config\.json; ignoring\n$/);
  expect(Decision.parse(JSON.parse(out.stdout)).hookSpecificOutput.permissionDecision).toBe("ask");
});
