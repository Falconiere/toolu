import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import {
  bashFixture,
  cursorEventName,
  editFixture,
  mcpFixture,
  patchFixture,
  postToolFixture,
  promptFixture,
  sessionFixture,
  toOpencodePermission,
  toStdin,
  writeFixture,
} from "../fixtures.ts";
import { HostOutputError, readHostOutcome } from "../hosts.ts";
import { createSandbox, type Sandbox } from "../sandbox.ts";
import { runHook } from "../spawn.ts";

const ROOT = resolve(import.meta.dir, "../../../../..");
const PLUGIN_ROOT = join(ROOT, "plugins/toolu");
/** toolu's PreToolUse hook: the protected-files gate is native in it since #260. */
const PRE_TOOLS = join(PLUGIN_ROOT, "hooks/dist/pre-tools.js");
const BLOCK = { version: 1, gates: { protectedFiles: { mode: "block" } } };

function protectedProject(): Sandbox {
  const sb = createSandbox({ git: true });
  sb.writeConfig("claude", "project", BLOCK);
  sb.writeConfig("codex", "project", BLOCK);
  return sb;
}

async function preToolEffect(sb: Sandbox, host: "claude" | "codex", file: string): Promise<string> {
  const stdin = toStdin(host, editFixture(sb.path(file), "a", "b"), { cwd: sb.project });
  const res = await runHook({
    host,
    sandbox: sb,
    pluginRoot: PLUGIN_ROOT,
    bundle: PRE_TOOLS,
    stdin,
  });
  return readHostOutcome(host, "PreToolUse", res).effect;
}

test.concurrent("claude: an Edit fixture on .env is denied by the real protected-files gate", async () => {
  using sb = protectedProject();
  expect(await preToolEffect(sb, "claude", ".env")).toBe("deny");
  expect(await preToolEffect(sb, "claude", "src/ok.ts")).not.toBe("deny");
});

test.concurrent("codex: the same Edit arrives as apply_patch and is denied too", async () => {
  using sb = protectedProject();
  const stdin = toStdin("codex", editFixture(sb.path(".env"), "a", "b"), { cwd: sb.project });
  expect(stdin).toMatchObject({ tool_name: "apply_patch", hook_event_name: "PreToolUse" });
  expect(await preToolEffect(sb, "codex", ".env")).toBe("deny");
  expect(await preToolEffect(sb, "codex", "src/ok.ts")).not.toBe("deny");
});

test.concurrent("a multi-file patch carries one header per file", () => {
  const fx = patchFixture([
    { op: "update", path: "src/a.ts", lines: ["-a", "+b"] },
    { op: "add", path: "src/b.ts", lines: ["export {};"] },
    { op: "delete", path: "old.ts" },
  ]);
  expect(fx).toMatchObject({ kind: "tool", toolName: "apply_patch" });
  expect(fx.kind === "tool" ? fx.toolInput.command : "").toBe(
    [
      "*** Begin Patch",
      "*** Update File: src/a.ts",
      "@@",
      "-a",
      "+b",
      "*** Add File: src/b.ts",
      "+export {};",
      "*** Delete File: old.ts",
      "*** End Patch",
    ].join("\n"),
  );
});

test.concurrent("claude stdin carries the documented envelope for tool, session and prompt events", () => {
  const ctx = { cwd: "/p", sessionId: "s1" };
  expect(toStdin("claude", bashFixture("ls"), ctx)).toEqual({
    session_id: "s1",
    cwd: "/p",
    hook_event_name: "PreToolUse",
    tool_name: "Bash",
    tool_input: { command: "ls" },
  });
  expect(
    toStdin("claude", postToolFixture(writeFixture("/p/a.ts", "x"), { success: true }), ctx),
  ).toMatchObject({
    hook_event_name: "PostToolUse",
    tool_name: "Write",
    tool_input: { file_path: "/p/a.ts", content: "x" },
    tool_response: { success: true },
  });
  expect(toStdin("claude", sessionFixture("resume"), ctx)).toMatchObject({
    hook_event_name: "SessionStart",
    source: "resume",
  });
  expect(toStdin("claude", promptFixture("hi"), ctx)).toMatchObject({
    hook_event_name: "UserPromptSubmit",
    prompt: "hi",
  });
  expect(toStdin("claude", mcpFixture("github", "get_me", {}), ctx)).toMatchObject({
    tool_name: "mcp__github__get_me",
  });
});

test.concurrent("codex stdin adds turn and call ids and turns Write into an Add File patch", () => {
  const stdin = toStdin("codex", writeFixture("/p/src/new.ts", "one\ntwo"), { cwd: "/p" });
  expect(stdin).toMatchObject({
    turn_id: "harness-turn",
    tool_use_id: "harness-call",
    tool_name: "apply_patch",
  });
  expect(stdin.tool_input).toEqual({
    command: "*** Begin Patch\n*** Add File: src/new.ts\n+one\n+two\n*** End Patch",
  });
});

test.concurrent("cursor stdin uses its native shell, MCP and preToolUse events", () => {
  const ctx = { cwd: "/p", sessionId: "s1" };
  expect(cursorEventName(bashFixture("ls"))).toBe("beforeShellExecution");
  expect(toStdin("cursor", bashFixture("ls"), ctx)).toMatchObject({
    hook_event_name: "beforeShellExecution",
    command: "ls",
    cwd: "/p",
  });
  expect(toStdin("cursor", mcpFixture("github", "get_me", { a: 1 }), ctx)).toMatchObject({
    hook_event_name: "beforeMCPExecution",
    tool_name: "get_me",
    tool_input: '{"a":1}',
    mcp_server_name: "github",
  });
  expect(toStdin("cursor", editFixture("/p/a.ts", "a", "b"), ctx)).toMatchObject({
    hook_event_name: "preToolUse",
    tool_name: "Edit",
  });
  expect(() => toStdin("cursor", sessionFixture("startup"), ctx)).toThrow(HostOutputError);
});

test.concurrent("opencode permission events cover edit, write and bash only", () => {
  expect(toOpencodePermission(editFixture("/p/a.ts", "a", "b"))).toEqual({
    sessionID: "harness-session",
    action: "edit",
    resources: ["/p/a.ts"],
    metadata: { file_path: "/p/a.ts" },
    effect: "allow",
  });
  expect(toOpencodePermission(bashFixture("ls"), "s2")).toMatchObject({
    sessionID: "s2",
    action: "bash",
    resources: ["ls"],
    metadata: { command: "ls" },
  });
  expect(toOpencodePermission(writeFixture("/p/b.ts", "x")).action).toBe("write");
  expect(() => toOpencodePermission(mcpFixture("github", "get_me", {}))).toThrow(HostOutputError);
});
