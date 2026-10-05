/**
 * jev's SessionStart and UserPromptSubmit bundles on OpenCode (#350), run the
 * way the OpenCode adapter spawns them: `TOOLU_HOST_OVERRIDE=opencode` and the
 * per-project data root as `TOOLU_CONFIG_DIR`.
 */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readlinkSync, unlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { bundlePath, entryArgv } from "@toolu/conformance/harness/entry-command";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch, type RunResult } from "@toolu/conformance/harness/spawn";
import { startupEnv } from "@toolu/conformance/harness/startup";
import { z } from "zod";

const PLUGIN = resolve(import.meta.dir, "../../..");
const STARTUP = "session-start";
const PROMPT = "user-prompt-submit";
const KEY = "local-test-key";
const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;

const OutputSchema = z.strictObject({
  hookSpecificOutput: z.strictObject({
    hookEventName: z.enum(["SessionStart", "UserPromptSubmit"]),
    additionalContext: z.string(),
  }),
});

function contextOf(res: RunResult): string {
  expect(res).toMatchObject({ exitCode: 0, stderr: "" });
  return OutputSchema.parse(JSON.parse(res.stdout)).hookSpecificOutput.additionalContext;
}

function dataRoot(sb: Sandbox): string {
  return join(sb.project, ".opencode/toolu/state");
}

function opencodeEnv(sb: Sandbox): EnvPatch {
  return {
    HOME: sb.home,
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_CONFIG_DIR: dataRoot(sb),
    TOOLU_PROJECT_DIR: sb.project,
    TOOLU_BUN: process.execPath,
    TYPESAFE_API_KEY: KEY,
    CLAUDE_CONFIG_DIR: undefined,
    CODEX_HOME: undefined,
  };
}

function hook(entry: string, sb: Sandbox, env: EnvPatch, stdin: object): Promise<RunResult> {
  return run(entryArgv("jev", entry, PLUGIN), {
    cwd: sb.project,
    env,
    stdin: JSON.stringify(stdin),
  });
}

function calledCommand(context: string): string | undefined {
  return context.split("you MUST call ")[1]?.split(" before the decision")[0];
}

test.concurrent("OpenCode startup names the native skill and a Bun command that ignores .env", async () => {
  using sb = createSandbox();
  const context = contextOf(
    await hook(STARTUP, sb, opencodeEnv(sb), {
      hook_event_name: "SessionStart",
      source: "startup",
    }),
  );
  const wrapper = join(dataRoot(sb), "jev/jev.sh");
  expect(readlinkSync(wrapper)).toBe(bundlePath(PLUGIN, "jev"));
  expect(calledCommand(context)).toBe(`${quote(process.execPath)} --no-env-file ${quote(wrapper)}`);
  expect(context).toContain('Syntax and linked examples: skill({ name: "jev-jev" }).');
  expect(context).not.toContain("skills/jev/SKILL.md");
  expect(context).not.toContain(KEY);
});

test.concurrent("OpenCode compaction relinks the wrapper and adds no second mandate", async () => {
  using sb = createSandbox();
  const env = opencodeEnv(sb);
  contextOf(await hook(STARTUP, sb, env, { source: "startup" }));
  const wrapper = join(dataRoot(sb), "jev/jev.sh");
  unlinkSync(wrapper);
  const res = await hook(STARTUP, sb, env, { hook_event_name: "SessionStart", source: "compact" });
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
  expect(readlinkSync(wrapper)).toBe(bundlePath(PLUGIN, "jev"));
});

test.concurrent("OpenCode prompt reminder carries the same command and skill", async () => {
  using sb = createSandbox();
  const env = opencodeEnv(sb);
  contextOf(await hook(STARTUP, sb, env, { source: "startup" }));
  const wrapper = join(dataRoot(sb), "jev/jev.sh");
  const context = contextOf(await hook(PROMPT, sb, env, { prompt: "rank these two designs" }));
  expect(calledCommand(context)).toBe(`${quote(process.execPath)} --no-env-file ${quote(wrapper)}`);
  expect(context).toContain('Syntax and linked examples: skill({ name: "jev-jev" }).');
  expect(context).not.toContain(KEY);
  expect((await hook(PROMPT, sb, env, { prompt: "ok" })).stdout).toBe("");
});

test.concurrent("OpenCode keeps a user override's own interpreter", async () => {
  using sb = createSandbox();
  const dst = join(dataRoot(sb), "jev/jev.sh");
  mkdirSync(join(dataRoot(sb), "jev"), { recursive: true });
  writeFileSync(dst, "#!/bin/sh\necho user-override\n", { mode: 0o755 });
  const context = contextOf(await hook(STARTUP, sb, opencodeEnv(sb), { source: "startup" }));
  expect(calledCommand(context)).toBe(quote(dst));
});

test.concurrent("Claude still restates the mandate after compaction", async () => {
  using sb = createSandbox();
  const env = { ...startupEnv("claude", sb, PLUGIN), TYPESAFE_API_KEY: KEY };
  const context = contextOf(await hook(STARTUP, sb, env, { source: "compact" }));
  expect(context).toContain(join(PLUGIN, "skills/jev/SKILL.md"));
  expect(context).not.toContain("--no-env-file");
  expect(existsSync(join(sb.home, ".claude/jev/jev.sh"))).toBe(true);
});
