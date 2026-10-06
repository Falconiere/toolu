/**
 * OpenCode context delivery (#341). Real SessionStart and UserPromptSubmit
 * bundles produce the text. A stand-in spawn is used only to prove abort and
 * a bad stdout are logged and do not change the turn.
 */
import { expect, test } from "bun:test";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { Hooks } from "@opencode-ai/plugin";
import type { Model, Part, UserMessage } from "@opencode-ai/sdk";
import type { SpawnOutcome, SpawnRequest } from "../../bootstrap/spawn.ts";
import { definedEnv } from "../../host/runtime-env.ts";
import type { HostBinding, LogLevel } from "../context.ts";
import { createContextHooks, type ContextPlan, type Spawner } from "../context-delivery.ts";
import { createTooluHooks } from "../hooks.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";
const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const RENAME = "Rename: find all refs (ast-grep + Grep on configs) before rewriting.";
const VAGUE = "Prompt too vague - specify what file/feature/error needs attention";
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

function textPart(sessionID: string, messageID: string, text: string): Part {
  return { id: "prt_user", sessionID, messageID, type: "text", text };
}

async function project(): Promise<string> {
  const root = await mkdtemp(join(tmpBase, "toolu-oc-context-"));
  await mkdir(join(root, ".opencode/toolu"), { recursive: true });
  await writeFile(
    join(root, ".opencode/toolu/plugins.json"),
    JSON.stringify({ version: 1, enabled: ["toolu"] }),
  );
  return root;
}

function binding(root: string, logged: Logged[]): HostBinding {
  const env = definedEnv(process.env);
  delete env.TOOLU_REPO_ROOT;
  delete env.TOOLU_ROOT;
  delete env.TOOLU_CONFIG_DIR;
  delete env.TOOLU_OPENCODE_HOME;
  env.XDG_CONFIG_HOME = join(root, ".xdg");
  env.TOOLU_BUN = process.execPath;
  return {
    directory: root,
    worktree: root,
    projectRoot: root,
    repoRootOption: REPO_ROOT,
    optionsError: undefined,
    env,
    log: (level, messageText) => {
      logged.push({ level, message: messageText });
      return Promise.resolve();
    },
  };
}

function requireHook<T>(hook: T | undefined, name: string): T {
  if (hook === undefined) throw new Error(`no ${name} hook`);
  return hook;
}

test("startup lines are appended once and the runtime notice stays on the log", async () => {
  const root = await project();
  const logged: Logged[] = [];
  const hooks = await createTooluHooks(binding(root, logged));
  const system = requireHook(hooks["experimental.chat.system.transform"], "system");
  const first = { system: ["keep-me"] };
  const second = { system: ["keep-me"] };
  await system({ sessionID: "ses_a", model: MODEL }, first);
  await system({ sessionID: "ses_b", model: MODEL }, second);
  expect(first.system[0]).toBe("keep-me");
  expect(second.system[0]).toBe("keep-me");
  expect(first.system.filter((line) => line.includes("Session Protocol"))).toHaveLength(1);
  expect(second.system.some((line) => line.includes("Session Protocol"))).toBe(true);
  expect(first.system.some((line) => line.includes("Toolu is on!"))).toBe(false);
  expect(
    logged.some((line) => line.level === "info" && line.message.includes("Toolu is on!")),
  ).toBe(true);
  await hooks.dispose?.();
});

async function promptOf(hooks: Hooks, sessionID: string, id: string, text: string) {
  const prompt = requireHook(hooks["chat.message"], "prompt");
  const parts: Part[] = [textPart(sessionID, id, text)];
  const output = { message: message(id, sessionID), parts };
  await prompt({ sessionID, messageID: id }, output);
  return output;
}

test("a rename reminder is appended, a trivial reply is not, and a vague verb does not throw", async () => {
  const root = await project();
  const hooks = await createTooluHooks(binding(root, []));
  const renamed = await promptOf(
    hooks,
    "ses_a",
    "msg_rename",
    "rename the parser in src/parser.ts",
  );
  expect(renamed.parts[0]).toMatchObject({ text: "rename the parser in src/parser.ts" });
  expect(renamed.parts.some((part) => part.type === "text" && part.text.includes(RENAME))).toBe(
    true,
  );
  const again = await promptOf(hooks, "ses_a", "msg_rename", "rename the parser in src/parser.ts");
  expect(
    again.parts.filter((part) => part.type === "text" && part.synthetic === true),
  ).toHaveLength(0);
  const ok = await promptOf(hooks, "ses_a", "msg_ok", "ok");
  expect(ok.parts).toHaveLength(1);
  const vague = await promptOf(hooks, "ses_a", "msg_fix", "fix");
  expect(vague.parts.some((part) => part.type === "text" && part.text === VAGUE)).toBe(true);
  await hooks.dispose?.();
});

test("compaction keeps unrelated context, the recovery instructions and the session id", async () => {
  const root = await project();
  const hooks = await createTooluHooks(binding(root, []));
  const compacting = requireHook(hooks["experimental.session.compacting"], "compacting");
  const output: { context: string[]; prompt?: string } = { context: ["keep-me"] };
  await compacting({ sessionID: "ses_child" }, output);
  const once = output.context.length;
  await compacting({ sessionID: "ses_child" }, output);
  expect(output.context[0]).toBe("keep-me");
  expect(output.context.some((line) => line.includes("Recover memories before continuing"))).toBe(
    true,
  );
  expect(output.context).toContain("toolu sessionID: ses_child");
  expect(output.prompt).toBeUndefined();
  expect(output.context).toHaveLength(once);
  await hooks.dispose?.();
});

test("a second load has no context hooks and disposing it leaves the first in place", async () => {
  const root = await project();
  const first = await createTooluHooks(binding(root, []));
  const second = await createTooluHooks(binding(root, []));
  expect(second["chat.message"]).toBeUndefined();
  await second.dispose?.();
  const system = requireHook(first["experimental.chat.system.transform"], "system");
  const output = { system: ["keep-me"] };
  await system({ model: MODEL }, output);
  expect(output.system.some((line) => line.includes("Session Protocol"))).toBe(true);
  await first.dispose?.();
});

test("not-ready installs no context hooks", async () => {
  const root = await project();
  const hooks = await createTooluHooks(binding(root, []), () => {
    throw new Error("boom");
  });
  expect(hooks["chat.message"]).toBeUndefined();
  expect(hooks["experimental.chat.system.transform"]).toBeUndefined();
  expect(hooks["experimental.session.compacting"]).toBeUndefined();
  await hooks.dispose?.();
});

function plan(spawnJobs: ContextPlan["prompt"]): ContextPlan {
  return {
    startupLines: ["startup-line"],
    notices: [],
    prompt: spawnJobs,
    compact: [],
    bun: process.execPath,
    projectRoot: "/tmp",
    env: {},
  };
}

const JOB = {
  plugin: "toolu",
  name: "user-prompt-submit",
  bundle: "/tmp/toolu/hooks/dist/user-prompt-submit.js",
  pluginDir: "/tmp/toolu",
  event: "UserPromptSubmit" as const,
};

function sessionIdOf(stdin: string): string {
  const parsed: unknown = JSON.parse(stdin);
  return parsed !== null &&
    typeof parsed === "object" &&
    "session_id" in parsed &&
    typeof parsed.session_id === "string"
    ? parsed.session_id
    : "";
}

function hangUntilAbort(aborted: string[]): Spawner {
  return (request: SpawnRequest) =>
    new Promise((resolve) => {
      const id = sessionIdOf(request.stdin);
      const finish = (outcome: SpawnOutcome): void => resolve(outcome);
      if (request.signal?.aborted === true) {
        aborted.push(id);
        finish({ status: "failed", reason: "startup cancelled" });
        return;
      }
      request.signal?.addEventListener("abort", () => {
        aborted.push(id);
        finish({ status: "failed", reason: "startup cancelled" });
      });
    });
}

const notJson: Spawner = () =>
  Promise.resolve({ status: "exited", exitCode: 0, stdout: "not json", stderr: "" });

test("deleting one session aborts only that spawn, and dispose aborts the rest", async () => {
  const aborted: string[] = [];
  const spawn = hangUntilAbort(aborted);
  const logged: Logged[] = [];
  const hooks = createContextHooks(
    plan([JOB]),
    (level, messageText) => {
      logged.push({ level, message: messageText });
      return Promise.resolve();
    },
    spawn,
  );
  const left: Part[] = [textPart("ses_a", "msg_a", "rename the parser")];
  const right: Part[] = [textPart("ses_b", "msg_b", "rename the parser")];
  const pendingLeft = hooks.prompt(
    { sessionID: "ses_a" },
    { message: message("msg_a", "ses_a"), parts: left },
  );
  const pendingRight = hooks.prompt(
    { sessionID: "ses_b" },
    { message: message("msg_b", "ses_b"), parts: right },
  );
  await hooks.event({
    event: { type: "session.deleted", properties: { info: { id: "ses_a" } } },
  });
  await pendingLeft;
  expect(aborted).toEqual(["ses_a"]);
  expect(left).toHaveLength(1);
  await hooks.dispose();
  await pendingRight;
  expect(aborted).toEqual(["ses_a", "ses_b"]);
  expect(right).toHaveLength(1);
});

test("session.deleted is also read from properties.sessionID and data.sessionID", async () => {
  const aborted: string[] = [];
  const hooks = createContextHooks(plan([JOB]), () => Promise.resolve(), hangUntilAbort(aborted));
  const pending = ["ses_prop", "ses_data"].map((id) =>
    hooks.prompt(
      { sessionID: id },
      { message: message(id, id), parts: [textPart(id, id, "rename the parser")] },
    ),
  );
  await hooks.event({ event: { type: "session.deleted", properties: { sessionID: "ses_prop" } } });
  await hooks.event({ event: { type: "session.deleted", data: { sessionID: "ses_data" } } });
  await Promise.all(pending);
  expect(aborted.toSorted()).toEqual(["ses_data", "ses_prop"]);
});

test("bad prompt stdout is logged and the user part stays", async () => {
  const spawn = notJson;
  const logged: Logged[] = [];
  const hooks = createContextHooks(
    plan([JOB]),
    (level, messageText) => {
      logged.push({ level, message: messageText });
      return Promise.resolve();
    },
    spawn,
  );
  const parts: Part[] = [textPart("ses_a", "msg_bad", "rename the parser")];
  await hooks.prompt({ sessionID: "ses_a" }, { message: message("msg_bad", "ses_a"), parts });
  expect(parts).toHaveLength(1);
  expect(logged.some((line) => line.level === "error" && line.message.includes("not JSON"))).toBe(
    true,
  );
});
