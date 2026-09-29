/**
 * Host-neutral hook event fixtures (#251) and their per-host stdin encodings.
 *
 * A fixture says what the agent did — edit a file, run a command, call an MCP
 * tool, start a session. `toStdin` renders it the way a given host delivers it:
 * Claude Code's envelope, Codex's (edits arrive as `apply_patch`), or Cursor's
 * native permission events. OpenCode runs in process, so `toOpencodePermission`
 * builds its permission-evaluation event instead.
 */
import { isAbsolute, relative } from "node:path";
import { HostOutputError, type CommandHost } from "./hosts.ts";

export type ToolFixture = {
  kind: "tool";
  event: "PreToolUse" | "PostToolUse";
  toolName: string;
  toolInput: Record<string, unknown>;
  toolResponse?: unknown;
  mcp?: { server: string; tool: string };
};
export type SessionSource = "startup" | "resume" | "clear" | "compact";
export type Fixture =
  | ToolFixture
  | { kind: "session"; event: "SessionStart"; source: SessionSource }
  | { kind: "prompt"; event: "UserPromptSubmit"; prompt: string };

export type PatchFile = { op: "add" | "update" | "delete"; path: string; lines?: string[] };
export type StdinContext = { cwd: string; sessionId?: string };
export type OpencodePermissionEvent = {
  sessionID: string;
  action: string;
  resources: string[];
  metadata: Record<string, unknown>;
  effect: "allow";
};

const DEFAULT_SESSION = "harness-session";

function tool(toolName: string, toolInput: Record<string, unknown>): ToolFixture {
  return { kind: "tool", event: "PreToolUse", toolName, toolInput };
}

export function editFixture(filePath: string, oldString: string, newString: string): Fixture {
  return tool("Edit", { file_path: filePath, old_string: oldString, new_string: newString });
}

export function writeFixture(filePath: string, content: string): Fixture {
  return tool("Write", { file_path: filePath, content });
}

const PATCH_SECTION: Record<PatchFile["op"], (path: string, lines: string[]) => string[]> = {
  add: (path, lines) => [`*** Add File: ${path}`, ...lines.map((line) => `+${line}`)],
  update: (path, lines) => [`*** Update File: ${path}`, "@@", ...lines],
  delete: (path) => [`*** Delete File: ${path}`],
};

function patchSection(file: PatchFile): string[] {
  return PATCH_SECTION[file.op](file.path, file.lines ?? []);
}

function patchText(files: PatchFile[]): string {
  return ["*** Begin Patch", ...files.flatMap(patchSection), "*** End Patch"].join("\n");
}

/** A Codex `apply_patch` call; `update` lines are raw diff lines (`-old`, `+new`). */
export function patchFixture(files: PatchFile[]): Fixture {
  return tool("apply_patch", { command: patchText(files) });
}

export function bashFixture(command: string): Fixture {
  return tool("Bash", { command });
}

export function mcpFixture(server: string, name: string, input: Record<string, unknown>): Fixture {
  return { ...tool(`mcp__${server}__${name}`, input), mcp: { server, tool: name } };
}

export function sessionFixture(source: SessionSource): Fixture {
  return { kind: "session", event: "SessionStart", source };
}

export function promptFixture(prompt: string): Fixture {
  return { kind: "prompt", event: "UserPromptSubmit", prompt };
}

/** The PostToolUse event for a tool fixture that already ran. */
export function postToolFixture(pre: Fixture, toolResponse: unknown): Fixture {
  if (pre.kind !== "tool") {
    throw new HostOutputError(`postToolFixture needs a tool fixture, got ${pre.kind}`);
  }
  return { ...pre, event: "PostToolUse", toolResponse };
}

function text(input: Record<string, unknown>, key: string): string {
  const value = input[key];
  return typeof value === "string" ? value : "";
}

function patchPath(filePath: string, cwd: string): string {
  const rel = relative(cwd, filePath);
  return rel.startsWith("..") || isAbsolute(rel) ? filePath : rel;
}

/** Codex reports every file edit as apply_patch. */
function codexTool(fx: ToolFixture, cwd: string): { name: string; input: Record<string, unknown> } {
  const path = patchPath(text(fx.toolInput, "file_path"), cwd);
  if (fx.toolName === "Edit") {
    const minus = text(fx.toolInput, "old_string")
      .split("\n")
      .map((line) => `-${line}`);
    const plus = text(fx.toolInput, "new_string")
      .split("\n")
      .map((line) => `+${line}`);
    return {
      name: "apply_patch",
      input: { command: patchText([{ op: "update", path, lines: [...minus, ...plus] }]) },
    };
  }
  if (fx.toolName === "Write") {
    const lines = text(fx.toolInput, "content").split("\n");
    return { name: "apply_patch", input: { command: patchText([{ op: "add", path, lines }]) } };
  }
  return { name: fx.toolName, input: fx.toolInput };
}

function hookStdin(
  host: "claude" | "codex",
  fx: Fixture,
  ctx: StdinContext,
): Record<string, unknown> {
  const base: Record<string, unknown> = {
    session_id: ctx.sessionId ?? DEFAULT_SESSION,
    cwd: ctx.cwd,
    hook_event_name: fx.event,
  };
  if (host === "codex") {
    base.turn_id = "harness-turn";
  }
  if (fx.kind === "session") {
    return { ...base, source: fx.source };
  }
  if (fx.kind === "prompt") {
    return { ...base, prompt: fx.prompt };
  }
  const { name, input } =
    host === "codex" ? codexTool(fx, ctx.cwd) : { name: fx.toolName, input: fx.toolInput };
  const out: Record<string, unknown> = { ...base, tool_name: name, tool_input: input };
  if (host === "codex") {
    out.tool_use_id = "harness-call";
  }
  if (fx.event === "PostToolUse") {
    out.tool_response = fx.toolResponse;
  }
  return out;
}

function cursorTool(fx: Fixture): ToolFixture {
  if (fx.kind !== "tool" || fx.event !== "PreToolUse") {
    throw new HostOutputError(
      `cursor: ${fx.event} is not modelled by the harness (only permission events are)`,
    );
  }
  return fx;
}

/** The native Cursor event `toStdin("cursor", fx)` renders. */
export function cursorEventName(fx: Fixture): string {
  const t = cursorTool(fx);
  if (t.toolName === "Bash") {
    return "beforeShellExecution";
  }
  return t.mcp ? "beforeMCPExecution" : "preToolUse";
}

function cursorStdin(fx: Fixture, ctx: StdinContext): Record<string, unknown> {
  const t = cursorTool(fx);
  const base = {
    conversation_id: ctx.sessionId ?? DEFAULT_SESSION,
    hook_event_name: cursorEventName(t),
    workspace_roots: [ctx.cwd],
  };
  if (t.toolName === "Bash") {
    return { ...base, command: text(t.toolInput, "command"), cwd: ctx.cwd, sandbox: false };
  }
  if (t.mcp) {
    return {
      ...base,
      tool_name: t.mcp.tool,
      tool_input: JSON.stringify(t.toolInput),
      mcp_server_name: t.mcp.server,
    };
  }
  return {
    ...base,
    tool_name: t.toolName,
    tool_input: t.toolInput,
    tool_use_id: "harness-call",
    cwd: ctx.cwd,
  };
}

/** Render `fx` as the stdin JSON `host` hands a command hook. */
export function toStdin(
  host: CommandHost,
  fx: Fixture,
  ctx: StdinContext,
): Record<string, unknown> {
  return host === "cursor" ? cursorStdin(fx, ctx) : hookStdin(host, fx, ctx);
}

/** The OpenCode permission.evaluate event for an edit, write or bash fixture. */
export function toOpencodePermission(
  fx: Fixture,
  sessionId = DEFAULT_SESSION,
): OpencodePermissionEvent {
  if (fx.kind === "tool" && (fx.toolName === "Edit" || fx.toolName === "Write")) {
    const filePath = text(fx.toolInput, "file_path");
    const action = fx.toolName === "Edit" ? "edit" : "write";
    return {
      sessionID: sessionId,
      action,
      resources: [filePath],
      metadata: { file_path: filePath },
      effect: "allow",
    };
  }
  if (fx.kind === "tool" && fx.toolName === "Bash") {
    const command = text(fx.toolInput, "command");
    return {
      sessionID: sessionId,
      action: "bash",
      resources: [command],
      metadata: { command },
      effect: "allow",
    };
  }
  throw new HostOutputError(
    `opencode: no permission action for ${fx.kind === "tool" ? fx.toolName : fx.event}`,
  );
}
