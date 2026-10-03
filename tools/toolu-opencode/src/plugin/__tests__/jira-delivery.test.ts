/**
 * jira on OpenCode (#351): the real jira and toolu bundles through the
 * adapter's hooks, and the published helper run the way the agent's bash runs
 * it (the `shell.env` environment) against a loopback HTTPS fixture posing as
 * acme.atlassian.net, replaying recorded Jira bodies. Credentials come from
 * the environment or the jira CLI login, never from a project `.env`, and
 * loading the plugin or its skill never reaches Jira.
 */
import { expect, test } from "bun:test";
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Config, Hooks } from "@opencode-ai/plugin";
import type { Part, UserMessage } from "@opencode-ai/sdk";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import type { EnvPatch, RunResult } from "@toolu/conformance/harness/spawn";
import { type HttpsFixture, startHttpsFixture } from "@toolu/conformance/https-fixture";
import { REPO_ROOT } from "../../bootstrap/__tests__/fixtures.ts";
import { isPlainRecord } from "../../surfaces/merge.ts";
import type { HostBinding } from "../context.ts";
import { createTooluHooks } from "../hooks.ts";
import { bashEnv, binding, hook, inShell, systemLines, type Logged } from "./jev-fixtures.ts";

const GENERATED = realpathSync(join(REPO_ROOT, "tools/toolu-opencode/generated"));
const ISSUE = readFileSync(
  join(REPO_ROOT, "plugins/jira/hooks/src/__tests__/fixtures/issue.json"),
  "utf8",
);
const INSTRUCTION = "jira (issue tracker)";
const SKILL = 'syntax: skill({ name: "jira-jira" })';
const BASE = "https://acme.atlassian.net";
const TOKEN = "env-pat-token";
const CONFIG_TOKEN = "cli-config-token";
/** Every JIRA_* a developer's shell might export, cleared; jira-cli discovery points at nothing. */
const NO_JIRA: EnvPatch = {
  JIRA_BASE_URL: undefined,
  JIRA_PAT: undefined,
  JIRA_EMAIL: undefined,
  JIRA_API_TOKEN: undefined,
  JIRA_API_VERSION: undefined,
  JIRA_CLI: undefined,
  JIRA_CLI_CONFIG: "/dev/null",
  NETRC: "/dev/null",
};
const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;

/** A project (optionally under a directory with a space) selecting `enabled`, whose `.env` names another host. */
function project(sb: Sandbox, enabled: string[], dir = "."): string {
  const root = join(sb.project, dir);
  mkdirSync(join(root, ".opencode/toolu"), { recursive: true });
  writeFileSync(
    join(root, ".opencode/toolu/plugins.json"),
    JSON.stringify({ version: 1, enabled }),
  );
  writeFileSync(join(root, ".env"), "JIRA_BASE_URL=https://media.example.net\n");
  return root;
}

function bindingAt(sb: Sandbox, root: string, logged: Logged[], extra: EnvPatch = {}): HostBinding {
  const base = binding(sb, logged, "");
  for (const [name, value] of Object.entries({ ...NO_JIRA, ...extra })) {
    if (value === undefined) delete base.env[name];
    else base.env[name] = value;
  }
  return { ...base, directory: root, worktree: root, projectRoot: root };
}

function count(lines: readonly string[], text: string): number {
  return lines.filter((line) => line.includes(text)).length;
}

function runCommand(lines: readonly string[]): string {
  const line = lines.find((text) => text.includes(INSTRUCTION)) ?? "";
  const command = line.split(", run ")[1]?.split(" <family>")[0];
  if (command === undefined) throw new Error(`no jira command in: ${line}`);
  return command;
}

/** The generated skill's OpenCode command, without its placeholder arguments. */
function skillCommand(): string {
  const skill = readFileSync(join(GENERATED, "skills/jira-jira/SKILL.md"), "utf8");
  const line = skill.split("# OpenCode\n")[1]?.split("\n")[0];
  if (line === undefined) throw new Error("no # OpenCode command in the generated skill");
  return line.replace(" [--api-version N] [--lean] <family> <action> [options]", "");
}

function message(id: string, sessionID: string): UserMessage {
  return {
    id,
    sessionID,
    role: "user",
    time: { created: 0 },
    agent: "build",
    model: { providerID: "probe", modelID: "scripted" },
  };
}

/** The synthetic text parts one user message gained. */
async function reminders(hooks: Hooks, sessionID: string, id: string, text: string) {
  const parts: Part[] = [{ id: "prt_user", sessionID, messageID: id, type: "text", text }];
  const output = { message: message(id, sessionID), parts };
  await hook(hooks, "chat.message")({ sessionID, messageID: id }, output);
  return output.parts.flatMap((part) =>
    part.type === "text" && part.synthetic === true ? [part.text] : [],
  );
}

test.concurrent("one instruction per request, none in compaction, the native skill, and no Jira request from loading", async () => {
  using sb = createSandbox();
  const root = project(sb, ["toolu", "jira"]);
  const fixture = await startHttpsFixture(["acme.atlassian.net", "media.example.net"]);
  // Startup children get the proxy and credentials: a request from loading would be recorded.
  const extra = { ...fixture.env, JIRA_BASE_URL: BASE, JIRA_PAT: TOKEN };
  const hooks = await createTooluHooks(bindingAt(sb, root, [], extra));
  try {
    const helper = join(root, ".opencode/toolu/state/jira/jira.sh");
    const requests = await Promise.all(
      ["ses_a", "ses_b", "ses_a"].map((sessionID) => systemLines(hooks, sessionID)),
    );
    for (const lines of requests) {
      expect(count(lines, INSTRUCTION)).toBe(1);
      expect(runCommand(lines)).toBe(`${quote(process.execPath)} --no-env-file ${quote(helper)}`);
      const line = lines.find((text) => text.includes(INSTRUCTION)) ?? "";
      expect(line).toContain(SKILL);
      expect(line).toContain("unless the user asked for that change");
    }
    const compaction = { context: ["keep-me"] };
    await hook(hooks, "experimental.session.compacting")({ sessionID: "ses_a" }, compaction);
    expect(compaction.context).toContain("toolu sessionID: ses_a");
    expect(count(compaction.context, INSTRUCTION)).toBe(0);
    const config: Config = {};
    await hook(hooks, "config")(config);
    const skills: unknown = Reflect.get(config, "skills");
    expect(isPlainRecord(skills) ? skills.paths : undefined).toContain(
      join(GENERATED, "skills/jira-jira"),
    );
    expect(fixture.connects).toEqual([]);
    expect(fixture.requests).toEqual([]);
  } finally {
    await hooks.dispose?.();
    await fixture.stop();
  }
});

type JiraBash = {
  command: string;
  fixture: HttpsFixture;
  inRoot: (line: string, patch?: EnvPatch) => Promise<RunResult>;
};

/**
 * A project under a directory with a space selecting toolu and jira, its
 * instruction's command, and bash with the `shell.env` env, Bun's directory
 * dropped from PATH, and Bearer credentials. `body` returns the outputs it
 * saw; none of them, no system line and no log may contain the token.
 */
async function withJiraBash(body: (bash: JiraBash) => Promise<string[]>): Promise<void> {
  using sb = createSandbox();
  const root = project(sb, ["toolu", "jira"], "my project");
  const logged: Logged[] = [];
  const fixture = await startHttpsFixture(["acme.atlassian.net", "media.example.net"]);
  const hooks = await createTooluHooks(bindingAt(sb, root, logged));
  try {
    const system = await systemLines(hooks, "ses_a");
    const env = {
      ...(await bashEnv(hooks, sb)),
      ...fixture.env,
      ...NO_JIRA,
      PATH: "/usr/bin:/bin",
      JIRA_BASE_URL: BASE,
      JIRA_PAT: TOKEN,
    };
    const outputs = await body({
      command: runCommand(system),
      fixture,
      inRoot: (line, patch = {}) => inShell({ ...sb, project: root }, line, { ...env, ...patch }),
    });
    const seen = [...outputs, ...system, ...logged.map((line) => line.message)].join("\n");
    expect(seen).not.toContain(TOKEN);
  } finally {
    await hooks.dispose?.();
    await fixture.stop();
  }
}

test.concurrent("a read and a mutation through the instruction's command", async () => {
  await withJiraBash(async ({ command, fixture, inRoot }) => {
    fixture.plan([{ body: ISSUE }]);
    const get = await inRoot(`${command} issue get ABC-1`);
    expect(get).toMatchObject({ exitCode: 0, stderr: "" });
    expect(JSON.parse(get.stdout)).toEqual(JSON.parse(ISSUE));
    expect(fixture.requests).toHaveLength(1);
    expect(fixture.requests[0]).toMatchObject({ method: "GET", path: "/rest/api/3/issue/ABC-1" });
    expect(fixture.requests[0]?.headers.host).toBe("acme.atlassian.net");
    expect(fixture.requests[0]?.headers.authorization).toBe(`Bearer ${TOKEN}`);

    fixture.plan([{ status: 201, body: '{"id":"10001"}' }]);
    const comment = await inRoot(`${command} issue comment ABC-1 'looks good'`);
    expect(comment).toMatchObject({ exitCode: 0, stderr: "" });
    expect(fixture.requests).toHaveLength(1);
    expect(fixture.requests[0]).toMatchObject({
      method: "POST",
      path: "/rest/api/3/issue/ABC-1/comment",
    });
    expect(JSON.parse(fixture.requests[0]?.body ?? "")).toEqual({
      body: {
        type: "doc",
        version: 1,
        content: [{ type: "paragraph", content: [{ type: "text", text: "looks good" }] }],
      },
    });
    return [get.stdout, get.stderr, comment.stdout, comment.stderr];
  });
});

test.concurrent("HTTP errors, missing credentials and the generated skill's command", async () => {
  await withJiraBash(async ({ command, fixture, inRoot }) => {
    fixture.plan([{ status: 400, body: '{"errorMessages":["Comment body is not valid"]}' }]);
    const rejected = await inRoot(`${command} issue comment ABC-1 'looks good'`);
    expect(rejected.exitCode).toBe(22);
    expect(rejected.stderr).toBe(`jira: HTTP 400 from ${BASE}/rest/api/3/issue/ABC-1/comment\n`);

    fixture.plan([{ status: 401, body: '{"errorMessages":["Unauthorized"]}' }]);
    const denied = await inRoot(`${command} issue get ABC-1`);
    expect(denied.exitCode).toBe(22);
    expect(denied.stderr).toBe(`jira: HTTP 401 from ${BASE}/rest/api/3/issue/ABC-1\n`);

    fixture.plan([{ body: ISSUE }]);
    const none = await inRoot(`${command} issue get ABC-1`, NO_JIRA);
    expect(none.exitCode).toBe(1);
    expect(none.stderr).toContain("jira: no Jira credentials found yet");
    expect(fixture.connects).toEqual([]);

    fixture.plan([{ body: ISSUE }]);
    const viaSkill = await inRoot(`${skillCommand()} issue get ABC-1`);
    expect(viaSkill).toMatchObject({ exitCode: 0, stderr: "" });
    expect(fixture.requests[0]).toMatchObject({ method: "GET", path: "/rest/api/3/issue/ABC-1" });
    return [rejected.stdout, rejected.stderr, denied.stdout, denied.stderr, viaSkill.stdout];
  });
});

test.concurrent("a project .env cannot send the jira CLI login to another host", async () => {
  using sb = createSandbox();
  const root = project(sb, ["toolu", "jira"]);
  const cliConfig = join(sb.home, "jira-cli.yml");
  writeFileSync(
    cliConfig,
    `installation: Cloud\nserver: ${BASE}\nlogin: dev@example.com\napi_token: ${CONFIG_TOKEN}\n`,
  );
  const fixture = await startHttpsFixture(["acme.atlassian.net", "media.example.net"]);
  const hooks = await createTooluHooks(bindingAt(sb, root, []));
  try {
    const command = runCommand(await systemLines(hooks, "ses_a"));
    const helper = join(root, ".opencode/toolu/state/jira/jira.sh");
    const env = {
      ...(await bashEnv(hooks, sb)),
      ...fixture.env,
      ...NO_JIRA,
      JIRA_CLI_CONFIG: cliConfig,
    };
    const whoami = async (line: string) => {
      fixture.plan([{ body: '{"accountId":"me"}' }]);
      const res = await inShell({ ...sb, project: root }, `${line} user whoami`, env);
      expect(res.exitCode).toBe(0);
      expect(fixture.requests).toHaveLength(1);
      return fixture.requests[0]?.headers;
    };
    const basic = `Basic ${Buffer.from(`dev@example.com:${CONFIG_TOKEN}`).toString("base64")}`;

    expect(await whoami(command)).toMatchObject({
      host: "acme.atlassian.net",
      authorization: basic,
    });
    // The hazard the instruction avoids: through its shebang Bun loads the .env host.
    expect(await whoami(quote(helper))).toMatchObject({
      host: "media.example.net",
      authorization: basic,
    });
  } finally {
    await hooks.dispose?.();
    await fixture.stop();
  }
});

/** A Jira prompt, the system lines and compaction for one selection. */
async function promptCase(enabled: string[]): Promise<void> {
  using sb = createSandbox();
  const root = project(sb, enabled);
  const hooks = await createTooluHooks(bindingAt(sb, root, []));
  try {
    const hints = await reminders(
      hooks,
      "ses_a",
      "msg_1",
      "check the jira ticket ABC-1 on the board",
    );
    expect(hints.join("\n")).not.toContain("Jira mentioned");
    const system = await systemLines(hooks, "ses_a");
    expect(count(system, INSTRUCTION)).toBe(enabled.includes("jira") ? 1 : 0);
    if (!enabled.includes("jira")) expect(count(system, "jira")).toBe(0);
    const compaction: { context: string[] } = { context: [] };
    await hook(hooks, "experimental.session.compacting")({ sessionID: "ses_a" }, compaction);
    expect(count(compaction.context, "jira")).toBe(0);
  } finally {
    await hooks.dispose?.();
  }
}

test.concurrent("toolu's prompt hint names no Jira skill on OpenCode, and deselected jira gives no instruction", async () => {
  await Promise.all([["toolu"], ["toolu", "jira"]].map(promptCase));
});
