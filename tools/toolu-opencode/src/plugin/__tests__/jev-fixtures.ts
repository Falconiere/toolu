/**
 * Shared by the hermetic and live Jev suites (#350): a project selecting Jev,
 * the host binding the adapter receives, and the bash environment `shell.env`
 * gives the agent.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Hooks } from "@opencode-ai/plugin";
import type { Model } from "@opencode-ai/sdk";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { REPO_ROOT } from "../../bootstrap/__tests__/fixtures.ts";
import { definedEnv } from "../../host/runtime-env.ts";
import type { HostBinding, LogLevel } from "../context.ts";

export const KEY = "fixture-key";
export const SECRET = "dotenv-secret";
export const MANDATE = "Jev is mandatory on every task";
export const REMINDER = "Jev is mandatory for this task";
export const ANSWER = {
  model: "jev-1.13.0",
  usage: { input_tokens: 3, output_tokens: 2 },
  answers: { q: { type: "noul", noul: 0.92 } },
};

export type Logged = { level: LogLevel; message: string };

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

export const SELECTION = JSON.stringify({ version: 1, enabled: ["toolu", "jev"] });

export function selectJev(sb: Sandbox, dotenv = false): void {
  mkdirSync(join(sb.project, ".opencode/toolu"), { recursive: true });
  writeFileSync(join(sb.project, ".opencode/toolu/plugins.json"), SELECTION);
  if (dotenv) writeFileSync(join(sb.project, ".env"), `TYPESAFE_API_KEY=${SECRET}\n`);
}

export function binding(sb: Sandbox, logged: Logged[], key: string): HostBinding {
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

export function hook<K extends keyof Hooks>(hooks: Hooks, name: K): NonNullable<Hooks[K]> {
  const found = hooks[name];
  if (found === undefined) throw new Error(`no ${name} hook`);
  return found;
}

export async function systemLines(hooks: Hooks, sessionID: string): Promise<string[]> {
  const output = { system: ["keep-me"] };
  await hook(hooks, "experimental.chat.system.transform")({ sessionID, model: MODEL }, output);
  return output.system;
}

export function calledCommand(context: string): string {
  const command = context.split("you MUST call ")[1]?.split(" before the decision")[0];
  if (command === undefined) throw new Error(`no command in: ${context}`);
  return command;
}

/** The env one bash call gets: the host's base, then toolu's `shell.env` additions. */
export async function bashEnv(hooks: Hooks, sb: Sandbox): Promise<Record<string, string>> {
  const output = { env: { HOME: sb.home, PATH: "/usr/bin:/bin" } };
  await hook(hooks, "shell.env")({ cwd: sb.project, sessionID: "ses_a", callID: "call_1" }, output);
  return output.env;
}

export function inShell(sb: Sandbox, command: string, env: EnvPatch) {
  return run(["/bin/sh", "-c", command], { cwd: sb.project, env });
}
