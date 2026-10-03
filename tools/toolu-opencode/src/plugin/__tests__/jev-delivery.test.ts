/**
 * Jev on OpenCode (#350): the real Jev bundles through the adapter's spawner
 * and hooks, the published wrapper run the way the agent's bash runs it (the
 * `shell.env` environment) against a loopback HTTPS fixture, and credential
 * checks that see only the environment, never a project `.env`.
 */
import { expect, test } from "bun:test";
import { mkdirSync, realpathSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Config, Hooks } from "@opencode-ai/plugin";
import type { Model, Part, UserMessage } from "@opencode-ai/sdk";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { startHttpsFixture } from "@toolu/conformance/https-fixture";
import { REPO_ROOT } from "../../bootstrap/__tests__/fixtures.ts";
import { spawnEntry } from "../../bootstrap/spawn.ts";
import { definedEnv } from "../../host/runtime-env.ts";
import { isPlainRecord } from "../../surfaces/merge.ts";
import type { HostBinding, LogLevel } from "../context.ts";
import { createTooluHooks } from "../hooks.ts";

const JEV = join(REPO_ROOT, "plugins/jev");
const GENERATED = realpathSync(join(REPO_ROOT, "tools/toolu-opencode/generated"));
const KEY = "fixture-key";
const SECRET = "dotenv-secret";
const MANDATE = "Jev is mandatory on every task";
const REMINDER = "Jev is mandatory for this task";
const NOTICE = "The Jev hook did not receive TYPESAFE_API_KEY";
const SKILL = 'Syntax and linked examples: skill({ name: "jev-jev" }).';
const ANSWER = {
  model: "jev-1.13.0",
  usage: { input_tokens: 3, output_tokens: 2 },
  answers: { q: { type: "noul", noul: 0.92 } },
};

type Logged = { level: LogLevel; message: string };

const MODEL: Model = {
  id: "scripted",
  providerID: "probe",
  api: { id: "scripted", url: "http://127.0.0.1", npm: "@ai-sdk/openai-compatible" },
  name: "scripted",
  capabilities: {
    temperature: false,
    reasoning: false,
    attachment: false,
    toolcall: true,
    input: { text: true, audio: false, image: false, video: false, pdf: false },
    output: { text: true, audio: false, image: false, video: false, pdf: false },
  },
  cost: { input: 0, output: 0, cache: { read: 0, write: 0 } },
  limit: { context: 1, output: 1 },
  status: "active",
  options: {},
  headers: {},
};

function selectJev(sb: Sandbox, dotenv = false): void {
  mkdirSync(join(sb.project, ".opencode/toolu"), { recursive: true });
  writeFileSync(
    join(sb.project, ".opencode/toolu/plugins.json"),
    JSON.stringify({ version: 1, enabled: ["toolu", "jev"] }),
  );
  if (dotenv) writeFileSync(join(sb.project, ".env"), `TYPESAFE_API_KEY=${SECRET}\n`);
}

function binding(sb: Sandbox, logged: Logged[], key: string): HostBinding {
  const env = definedEnv(process.env);
  for (const name of ["TOOLU_REPO_ROOT", "TOOLU_ROOT", "TOOLU_CONFIG_DIR", "TOOLU_OPENCODE_HOME"])
    delete env[name];
  env.HOME = sb.home;
  env.XDG_CONFIG_HOME = join(sb.home, ".config");
  env.TOOLU_BUN = process.execPath;
  env.TYPESAFE_API_KEY = key;
  return {
    directory: sb.project,
    worktree: sb.project,
    projectRoot: sb.project,
    repoRootOption: REPO_ROOT,
    optionsError: undefined,
    env,
    log: (level, text) => {
      logged.push({ level, message: text });
      return Promise.resolve();
    },
  };
}

function hook<K extends keyof Hooks>(hooks: Hooks, name: K): NonNullable<Hooks[K]> {
  const found = hooks[name];
  if (found === undefined) throw new Error(`no ${name} hook`);
  return found;
}

function skillPaths(config: Config): unknown {
  const skills: unknown = Reflect.get(config, "skills");
  return isPlainRecord(skills) ? skills.paths : undefined;
}

async function systemLines(hooks: Hooks, sessionID: string): Promise<string[]> {
  const output = { system: ["keep-me"] };
  await hook(hooks, "experimental.chat.system.transform")({ sessionID, model: MODEL }, output);
  return output.system;
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

function count(lines: readonly string[], text: string): number {
  return lines.filter((line) => line.includes(text)).length;
}

function calledCommand(context: string): string {
  const command = context.split("you MUST call ")[1]?.split(" before the decision")[0];
  if (command === undefined) throw new Error(`no command in: ${context}`);
  return command;
}

/** The env one bash call gets: the host's base, then toolu's `shell.env` additions. */
async function bashEnv(hooks: Hooks, sb: Sandbox): Promise<Record<string, string>> {
  const output = { env: { HOME: sb.home, PATH: "/usr/bin:/bin" } };
  await hook(hooks, "shell.env")({ cwd: sb.project, sessionID: "ses_a", callID: "call_1" }, output);
  return output.env;
}

function inShell(sb: Sandbox, command: string, env: EnvPatch) {
  return run(["/bin/sh", "-c", command], { cwd: sb.project, env });
}

test.concurrent("a project .env never reaches a Jev hook the adapter spawns", async () => {
  using sb = createSandbox({ files: { ".env": `TYPESAFE_API_KEY=${SECRET}\n` } });
  const env = {
    HOME: sb.home,
    PATH: "/usr/bin:/bin",
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_CONFIG_DIR: join(sb.project, ".opencode/toolu/state"),
  };
  const spawn = (bundle: string, stdin: object) =>
    spawnEntry({
      bun: process.execPath,
      bundle: join(JEV, "hooks/dist", bundle),
      cwd: sb.project,
      env,
      stdin: JSON.stringify(stdin),
      deadlineMs: 30_000,
      signal: undefined,
    });
  for (const outcome of [
    await spawn("session-start.js", { source: "startup" }),
    await spawn("user-prompt-submit.js", { prompt: "rank these two designs" }),
  ]) {
    expect(outcome).toMatchObject({ status: "exited", exitCode: 0, stderr: "" });
    const stdout = outcome.status === "exited" ? outcome.stdout : "";
    expect(stdout).toContain(NOTICE);
    expect(stdout).not.toContain(SECRET);
  }
});

test.concurrent("one mandate per request, one reminder per real prompt, none in compaction", async () => {
  using sb = createSandbox();
  selectJev(sb);
  const hooks = await createTooluHooks(binding(sb, [], KEY));
  try {
    const requests = await Promise.all(
      ["ses_a", "ses_b", "ses_a"].map((sessionID) => systemLines(hooks, sessionID)),
    );
    for (const lines of requests) {
      expect(lines[0]).toBe("keep-me");
      expect(count(lines, MANDATE)).toBe(1);
    }
    expect(
      count(await reminders(hooks, "ses_a", "msg_1", "rank these two designs"), REMINDER),
    ).toBe(1);
    expect(await reminders(hooks, "ses_a", "msg_1", "rank these two designs")).toEqual([]);
    expect(count(await reminders(hooks, "ses_a", "msg_2", "ok"), REMINDER)).toBe(0);
    expect(count(await reminders(hooks, "ses_b", "msg_3", "pick a parser"), REMINDER)).toBe(1);
    const compaction = { context: ["keep-me"] };
    await hook(hooks, "experimental.session.compacting")({ sessionID: "ses_a" }, compaction);
    expect(compaction.context).toContain("toolu sessionID: ses_a");
    expect(count(compaction.context, "Jev is mandatory")).toBe(0);
  } finally {
    await hooks.dispose?.();
  }
});

test.concurrent("a typed judgment runs through the published wrapper in the agent's bash", async () => {
  using sb = createSandbox();
  selectJev(sb);
  const fixture = await startHttpsFixture(["api.typesafe.ai"]);
  const hooks = await createTooluHooks(binding(sb, [], KEY));
  try {
    const mandate = (await systemLines(hooks, "ses_a")).find((line) => line.includes(MANDATE));
    if (mandate === undefined) throw new Error("no Jev mandate");
    const wrapper = join(sb.project, ".opencode/toolu/state/jev/jev.sh");
    const command = calledCommand(mandate);
    expect(command).toBe(`'${process.execPath}' --no-env-file '${wrapper}'`);
    expect(mandate).toContain(SKILL);
    expect(mandate).not.toContain(NOTICE);
    const prompt = await reminders(hooks, "ses_a", "msg_1", "rank these two designs");
    expect(prompt.filter((text) => text.includes(REMINDER)).map(calledCommand)).toEqual([command]);
    const config: Config = {};
    await hook(hooks, "config")(config);
    expect(skillPaths(config)).toContain(join(GENERATED, "skills/jev-jev"));

    const env = { ...(await bashEnv(hooks, sb)), ...fixture.env, TYPESAFE_API_KEY: KEY };
    fixture.plan([{ body: JSON.stringify(ANSWER) }]);
    const judged = await inShell(sb, `${command} noul probe -s evidence`, env);
    expect(judged).toMatchObject({ exitCode: 0, stderr: "" });
    expect(JSON.parse(judged.stdout)).toEqual({ q: { type: "noul", noul: 0.92 } });
    expect(fixture.requests[0]?.headers.authorization).toBe(`Bearer ${KEY}`);

    fixture.plan([{ status: 401, body: '{"error":"invalid key"}' }]);
    const refused = await inShell(sb, `${command} noul probe -s evidence`, env);
    expect(refused.exitCode).toBe(22);
    expect(refused.stdout).not.toContain('"noul"');
  } finally {
    await hooks.dispose?.();
    await fixture.stop();
  }
});

test.concurrent("credentials are checked for presence only and never read from .env", async () => {
  using sb = createSandbox();
  selectJev(sb, true);
  const logged: Logged[] = [];
  const fixture = await startHttpsFixture(["api.typesafe.ai"]);
  const hooks = await createTooluHooks(binding(sb, logged, ""));
  try {
    const system = await systemLines(hooks, "ses_a");
    const prompt = await reminders(hooks, "ses_a", "msg_1", "rank these two designs");
    const mandate = system.find((line) => line.includes(MANDATE));
    if (mandate === undefined) throw new Error("no Jev mandate");
    expect(mandate).toContain(NOTICE);
    expect(count(prompt, NOTICE)).toBe(1);
    const seen = [...system, ...prompt, ...logged.map((line) => line.message)].join("\n");
    expect(seen).not.toContain(SECRET);

    fixture.plan([{ body: JSON.stringify(ANSWER) }]);
    const env = { ...(await bashEnv(hooks, sb)), ...fixture.env, TYPESAFE_API_KEY: undefined };
    const res = await inShell(sb, `${calledCommand(mandate)} noul probe -s evidence`, env);
    expect(res).toMatchObject({ exitCode: 1, stdout: "", stderr: "jev: TYPESAFE_API_KEY unset\n" });
    expect(fixture.requests).toHaveLength(0);
  } finally {
    await hooks.dispose?.();
    await fixture.stop();
  }
});
