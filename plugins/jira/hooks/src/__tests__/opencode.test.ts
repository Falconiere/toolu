/**
 * jira on OpenCode (#351), run the way the OpenCode adapter and the agent's
 * bash run it: `TOOLU_HOST_OVERRIDE=opencode`, the per-project data root as
 * `TOOLU_CONFIG_DIR`, and `.opencode` as the project state directory. The
 * SessionStart bundle gives the instruction; `plan run` keeps a project
 * `.env` out of its nested "$JIRA" calls.
 */
import { afterAll, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, readlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { bundlePath, entryArgv } from "@toolu/conformance/harness/entry-command";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch, type RunResult } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
import { BUNDLE, jiraEnv, startJira } from "./harness.ts";

const PLUGIN = resolve(import.meta.dir, "../../..");
const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
const SECRET = "dotenv-pat-secret";

const OutputSchema = z.strictObject({
  hookSpecificOutput: z.strictObject({
    hookEventName: z.literal("SessionStart"),
    additionalContext: z.string(),
  }),
  // The deprecation notice (#403) rides beside the instruction at a start.
  systemMessage: z.literal(
    "jira is deprecated and will be removed in v8.0.0; uninstall with: npx @toolu/plugins remove jira --host opencode --yes",
  ),
});

function contextOf(res: RunResult): string {
  expect(res).toMatchObject({ exitCode: 0, stderr: "" });
  return OutputSchema.parse(JSON.parse(res.stdout)).hookSpecificOutput.additionalContext;
}

function helper(sb: Sandbox): string {
  return join(sb.project, ".opencode/toolu/state/jira/jira.sh");
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
  return run(entryArgv("jira", "session-start", PLUGIN), {
    cwd: sb.project,
    env: opencodeEnv(sb),
    stdin: JSON.stringify({ source }),
  });
}

test.concurrent("startup publishes the helper and names it with Bun and the native skill", async () => {
  using sb = createSandbox();
  const context = contextOf(await startup(sb, "startup"));
  expect(readlinkSync(helper(sb))).toBe(bundlePath(PLUGIN, "jira"));
  expect(context).toContain(
    `run ${quote(process.execPath)} --no-env-file ${quote(helper(sb))} <family> <action> [options] (syntax: skill({ name: "jira-jira" }))`,
  );
  expect(context).toContain("never from .env");
});

test.concurrent("the instruction makes reads direct and writes the user's call", async () => {
  using sb = createSandbox();
  const context = contextOf(await startup(sb, "startup"));
  expect(context).toContain("Read-only calls (search, issue get");
  expect(context).toContain(
    "Never create, update, comment on, transition, assign or delete an issue, or change a sprint, worklog or attachment, unless the user asked for that change",
  );
  // Bounded: one paragraph, well under the startup context budget.
  expect(context.split("\n")).toHaveLength(1);
  expect(context.split(/\s+/).length).toBeLessThan(130);
});

test.concurrent("compaction relinks the helper and prints nothing", async () => {
  using sb = createSandbox();
  const res = await startup(sb, "compact");
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
  expect(readlinkSync(helper(sb))).toBe(bundlePath(PLUGIN, "jira"));
});

test.concurrent("a user's own jira.sh is kept and named alone", async () => {
  using sb = createSandbox();
  mkdirSync(join(helper(sb), ".."), { recursive: true });
  writeFileSync(helper(sb), "#!/bin/sh\necho mine\n");
  chmodSync(helper(sb), 0o755);
  const context = contextOf(await startup(sb, "startup"));
  expect(context).toContain(`run ${quote(helper(sb))} <family> <action>`);
  expect(context).not.toContain("--no-env-file");
});

test.concurrent("a data root with a space and a quote is quoted for the shell", async () => {
  using sb = createSandbox();
  const root = join(sb.project, "it's a dir");
  const res = await run(entryArgv("jira", "session-start", PLUGIN), {
    cwd: sb.project,
    env: { ...opencodeEnv(sb), TOOLU_CONFIG_DIR: root },
    stdin: "not json",
  });
  const path = join(root, "jira/jira.sh");
  expect(contextOf(res)).toContain(`--no-env-file ${quote(path)} <family>`);
  const usage = await run(
    ["/bin/sh", "-c", `${quote(process.execPath)} --no-env-file ${quote(path)}`],
    { cwd: sb.project, env: { PATH: "/usr/bin:/bin" } },
  );
  expect(usage.exitCode).toBe(1);
  expect(usage.stderr).toContain("plan         init|run|status|path");
});

const h = await startJira();
afterAll(() => h.fixture.stop());

/** A git repo whose `.env` holds a PAT, and a plan whose one check calls "$JIRA" back. */
function planRepo(sb: Sandbox): string {
  const repo = join(sb.project, "repo");
  mkdirSync(repo, { recursive: true });
  spawnSync("git", ["init", "-q", repo]);
  writeFileSync(join(repo, ".env"), `JIRA_PAT=${SECRET}\n`);
  const steps = [{ id: "who", title: "whoami", check: '"$JIRA" user whoami >/dev/null' }];
  writeFileSync(
    join(repo, "plan.md"),
    `# t\n\n**Issue:** ABC-123   **Topic:** t\n\n## Steps (machine-readable)\n\n\`\`\`json\n${JSON.stringify(steps)}\n\`\`\`\n`,
  );
  return repo;
}

/** `plan run` as the instruction runs it (`--no-env-file`), with Basic credentials in the env. */
async function planRun(repo: string, host: EnvPatch): Promise<string[]> {
  h.fixture.plan([{ body: '{"accountId":"me"}' }]);
  const env = jiraEnv(h.fixture, {
    JIRA_PAT: undefined,
    JIRA_EMAIL: "dev@example.com",
    JIRA_API_TOKEN: "basic-token",
    JIRA_CLI: BUNDLE,
    ...host,
  });
  const res = await run([process.execPath, "--no-env-file", BUNDLE, "plan", "run", "plan.md"], {
    cwd: repo,
    env,
  });
  expect(res.stderr).not.toContain(SECRET);
  expect(res.stdout).not.toContain(SECRET);
  return h.fixture.requests.map((request) => request.headers["authorization"] ?? "");
}

const BASIC = `Basic ${Buffer.from("dev@example.com:basic-token").toString("base64")}`;

test("on OpenCode the probe and the nested check ignore the project .env", async () => {
  using sb = createSandbox();
  const repo = planRepo(sb);
  const auth = await planRun(repo, {
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    BUN_OPTIONS: "--smol",
  });
  // The probe, then the check.
  expect(auth).toEqual([BASIC, BASIC]);
  expect(existsSync(join(repo, ".opencode/tmp/plan-ledger/jira-ABC-123.json"))).toBe(true);
});

test("off OpenCode the nested calls keep today's environment", async () => {
  using sb = createSandbox();
  const repo = planRepo(sb);
  const auth = await planRun(repo, { TOOLU_HOST_OVERRIDE: "claude" });
  // Unchanged on Claude Code: the shebang loads the .env (the cross-host follow-up).
  expect(auth).toEqual([`Bearer ${SECRET}`, `Bearer ${SECRET}`]);
  expect(existsSync(join(repo, ".claude/tmp/plan-ledger/jira-ABC-123.json"))).toBe(true);
});
