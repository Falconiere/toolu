/**
 * context7's SessionStart bundle on OpenCode (#348), run the way the OpenCode
 * adapter spawns it: `TOOLU_HOST_OVERRIDE=opencode` and the per-project data
 * root as `TOOLU_CONFIG_DIR`.
 */
import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, readlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch, type RunResult } from "@toolu/conformance/harness/spawn";
import { z } from "zod";

const PLUGIN = resolve(import.meta.dir, "../../..");
const STARTUP = join(PLUGIN, "hooks/dist/session-start.js");
const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;

const OutputSchema = z.strictObject({
  hookSpecificOutput: z.strictObject({
    hookEventName: z.literal("SessionStart"),
    additionalContext: z.string(),
  }),
  // The deprecation notice (#403) rides beside the instruction at a start.
  systemMessage: z.literal(
    "context7 is deprecated and will be removed in v8.0.0; uninstall with: npx @toolu/plugins remove context7 --host opencode --yes",
  ),
});

function contextOf(res: RunResult): string {
  expect(res).toMatchObject({ exitCode: 0, stderr: "" });
  return OutputSchema.parse(JSON.parse(res.stdout)).hookSpecificOutput.additionalContext;
}

function helper(sb: Sandbox): string {
  return join(sb.project, ".opencode/toolu/state/context7/search.sh");
}

function opencodeEnv(sb: Sandbox): EnvPatch {
  return {
    HOME: sb.home,
    PATH: "/usr/bin:/bin",
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_CONFIG_DIR: join(sb.project, ".opencode/toolu/state"),
    TOOLU_PROJECT_DIR: sb.project,
    CLAUDE_CONFIG_DIR: undefined,
    CODEX_HOME: undefined,
  };
}

function startup(sb: Sandbox, source: string): Promise<RunResult> {
  return run([process.execPath, STARTUP], {
    cwd: sb.project,
    env: opencodeEnv(sb),
    stdin: JSON.stringify({ source }),
  });
}

test.concurrent("startup publishes the helper and names it with Bun and the native skill", async () => {
  using sb = createSandbox();
  const context = contextOf(await startup(sb, "startup"));
  expect(readlinkSync(helper(sb))).toBe(join(PLUGIN, "hooks/dist/search.js"));
  expect(context).toContain(
    `you MUST run ${quote(process.execPath)} --no-env-file ${quote(helper(sb))} FIRST`,
  );
  expect(context).toContain('Syntax: skill({ name: "context7-context7" }).');
  expect(context).toContain("never from .env");
});

test.concurrent("compaction relinks the helper and prints nothing", async () => {
  using sb = createSandbox();
  const res = await startup(sb, "compact");
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
  expect(readlinkSync(helper(sb))).toBe(join(PLUGIN, "hooks/dist/search.js"));
});

test.concurrent("a user's own search.sh is kept and named alone", async () => {
  using sb = createSandbox();
  mkdirSync(join(helper(sb), ".."), { recursive: true });
  writeFileSync(helper(sb), "#!/bin/sh\necho mine\n");
  chmodSync(helper(sb), 0o755);
  const context = contextOf(await startup(sb, "startup"));
  expect(context).toContain(`you MUST run ${quote(helper(sb))} FIRST`);
  expect(context).not.toContain("--no-env-file");
});

test.concurrent("a data root with a space and a quote is quoted for the shell", async () => {
  using sb = createSandbox();
  const root = join(sb.project, "it's a dir");
  const res = await run([process.execPath, STARTUP], {
    cwd: sb.project,
    env: { ...opencodeEnv(sb), TOOLU_CONFIG_DIR: root },
    stdin: "not json",
  });
  const path = join(root, "context7/search.sh");
  expect(contextOf(res)).toContain(`--no-env-file ${quote(path)} FIRST`);
  const usage = await run(
    ["/bin/sh", "-c", `${quote(process.execPath)} --no-env-file ${quote(path)}`],
    {
      cwd: sb.project,
      env: { PATH: "/usr/bin:/bin" },
    },
  );
  expect(usage.exitCode).toBe(1);
  expect(usage.stderr).toContain("Context7 CLI");
});
