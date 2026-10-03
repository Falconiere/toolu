/**
 * Jev on OpenCode (#350): the real Jev bundles through the adapter's spawner
 * and hooks, the published wrapper run the way the agent's bash runs it (the
 * `shell.env` environment) against a loopback HTTPS fixture, and credential
 * checks that see only the environment, never a project `.env`.
 */
import { expect, test } from "bun:test";
import { realpathSync } from "node:fs";
import { join } from "node:path";
import type { Config, Hooks } from "@opencode-ai/plugin";
import type { Part, UserMessage } from "@opencode-ai/sdk";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { startHttpsFixture } from "@toolu/conformance/https-fixture";
import { REPO_ROOT } from "../../bootstrap/__tests__/fixtures.ts";
import { spawnEntry } from "../../bootstrap/spawn.ts";
import { isPlainRecord } from "../../surfaces/merge.ts";
import { createTooluHooks } from "../hooks.ts";
import {
  ANSWER,
  KEY,
  MANDATE,
  REMINDER,
  SECRET,
  bashEnv,
  binding,
  calledCommand,
  hook,
  inShell,
  selectJev,
  systemLines,
  type Logged,
} from "./jev-fixtures.ts";

const JEV = join(REPO_ROOT, "plugins/jev");
const GENERATED = realpathSync(join(REPO_ROOT, "tools/toolu-opencode/generated"));
const NOTICE = "The Jev hook did not receive TYPESAFE_API_KEY";
const SKILL = 'Syntax and linked examples: skill({ name: "jev-jev" }).';

function skillPaths(config: Config): unknown {
  const skills: unknown = Reflect.get(config, "skills");
  return isPlainRecord(skills) ? skills.paths : undefined;
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
