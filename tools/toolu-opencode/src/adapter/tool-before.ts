/**
 * OpenCode `tool.execute.before` → toolu's native pre-tool gates (#336, #337).
 *
 * A throw is the host's only refusal (probes `deny.*`): it stops the call before
 * any side effect and becomes the tool error the model sees. The pinned host has
 * no proven gate-generated ask channel, so gateMode degrades asks by class and
 * a remaining registry ask throws. Tools this module does not recognize pass
 * through; the user's own permission rules still apply.
 */
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import type { Hooks } from "@opencode-ai/plugin";
import { applyPatchRecords } from "@toolu/core/state";
import { createGateDecider, type PermissionEvaluateHandlerOptions } from "./evaluate.ts";
import type { PermissionContext, PermissionMapping } from "./permission-map.ts";
import type { ToolAdviceStore } from "./tool-advice.ts";

export type ToolBefore = NonNullable<Hooks["tool.execute.before"]>;
export type ToolCall = { tool: string; sessionID: string; callID: string };

type Args = Record<string, unknown>;

function deny(tool: string, problem: string): PermissionMapping {
  return { kind: "deny", reason: `toolu: ${tool} call ${problem}` };
}

function request(
  call: ToolCall,
  ctx: PermissionContext,
  toolName: string,
  input: Args,
): PermissionMapping {
  return {
    kind: "request",
    request: {
      session_id: call.sessionID,
      tool_use_id: call.callID,
      cwd: ctx.cwd,
      tool_name: toolName,
      tool_input: input,
    },
  };
}

function argsOf(value: unknown): Args | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return undefined;
  return Object.fromEntries(Object.entries(value));
}

function stringField(args: Args, key: string): string | undefined {
  const value = args[key];
  return typeof value === "string" ? value : undefined;
}

function presentWrong(args: Args, key: string, ok: boolean): boolean {
  return key in args && !ok;
}

/** Host `sanitize`: characters outside `[a-zA-Z0-9_-]` become `_`. */
function sanitize(value: string): string {
  return value.replace(/[^a-zA-Z0-9_-]/g, "_");
}

/** Longest configured server prefix of `sanitize(server)_<tool>`. */
function mcpToolName(tool: string, servers: readonly string[]): string | undefined {
  const prefixes = [...new Set(servers.map(sanitize).filter((name) => name !== ""))].toSorted(
    (a, b) => b.length - a.length,
  );
  for (const prefix of prefixes) {
    const rest = tool.startsWith(`${prefix}_`) ? tool.slice(prefix.length + 1) : "";
    if (rest !== "") return `mcp__${prefix}__${rest}`;
  }
  return undefined;
}

function bash(call: ToolCall, args: Args, ctx: PermissionContext): PermissionMapping {
  const command = stringField(args, "command");
  if (command === undefined || command.length === 0)
    return deny(call.tool, "missing a string command");
  if (presentWrong(args, "timeout", typeof args.timeout === "number")) {
    return deny(call.tool, "timeout must be a number");
  }
  const workdir = stringField(args, "workdir");
  if (presentWrong(args, "workdir", workdir !== undefined && workdir.length > 0)) {
    return deny(call.tool, "workdir must be a non-empty string");
  }
  const input: Args = { command };
  if (typeof args.timeout === "number") input.timeout = args.timeout;
  const cwd = workdir === undefined ? ctx.cwd : resolve(ctx.cwd, workdir);
  return request(call, { ...ctx, cwd }, "Bash", input);
}

function edit(call: ToolCall, args: Args, ctx: PermissionContext): PermissionMapping {
  const filePath = stringField(args, "filePath");
  if (filePath === undefined || filePath.length === 0) {
    return deny(call.tool, "missing a string filePath");
  }
  const oldString = stringField(args, "oldString");
  const newString = stringField(args, "newString");
  if (oldString === undefined || newString === undefined) {
    return deny(call.tool, "missing string oldString and newString");
  }
  if (presentWrong(args, "replaceAll", typeof args.replaceAll === "boolean")) {
    return deny(call.tool, "replaceAll must be a boolean");
  }
  const input: Args = { file_path: filePath, old_string: oldString, new_string: newString };
  if (typeof args.replaceAll === "boolean") input.replace_all = args.replaceAll;
  return request(call, ctx, "Edit", input);
}

function write(call: ToolCall, args: Args, ctx: PermissionContext): PermissionMapping {
  const filePath = stringField(args, "filePath");
  if (filePath === undefined || filePath.length === 0) {
    return deny(call.tool, "missing a string filePath");
  }
  if (typeof args.content !== "string") return deny(call.tool, "missing a string content");
  return request(call, ctx, "Write", { file_path: filePath, content: args.content });
}

function read(call: ToolCall, args: Args, ctx: PermissionContext): PermissionMapping {
  const filePath = stringField(args, "filePath");
  if (filePath === undefined || filePath.length === 0) {
    return deny(call.tool, "missing a string filePath");
  }
  if (presentWrong(args, "offset", typeof args.offset === "number")) {
    return deny(call.tool, "offset must be a number");
  }
  if (presentWrong(args, "limit", typeof args.limit === "number")) {
    return deny(call.tool, "limit must be a number");
  }
  const input: Args = { file_path: filePath };
  if (typeof args.offset === "number") input.offset = args.offset;
  if (typeof args.limit === "number") input.limit = args.limit;
  return request(call, ctx, "Read", input);
}

function search(
  call: ToolCall,
  args: Args,
  ctx: PermissionContext,
  toolName: "Grep" | "Glob",
): PermissionMapping {
  const pattern = stringField(args, "pattern");
  if (pattern === undefined || pattern.length === 0) {
    return deny(call.tool, "missing a string pattern");
  }
  if (presentWrong(args, "path", typeof args.path === "string")) {
    return deny(call.tool, "path must be a string");
  }
  if (toolName === "Grep" && presentWrong(args, "include", typeof args.include === "string")) {
    return deny(call.tool, "include must be a string");
  }
  const input: Args = { pattern };
  if (typeof args.path === "string") input.path = args.path;
  if (toolName === "Grep" && typeof args.include === "string") {
    input.include = args.include;
    input.glob = args.include;
  }
  return request(call, ctx, toolName, input);
}

/** Undefined when the patch is malformed or the parser throws. */
function readablePatch(patchText: string): boolean {
  try {
    return applyPatchRecords(patchText) !== undefined;
  } catch {
    return false;
  }
}

function patch(call: ToolCall, args: Args, ctx: PermissionContext): PermissionMapping {
  const patchText = stringField(args, "patchText");
  if (patchText === undefined || patchText.length === 0) {
    return deny(call.tool, "missing a string patchText");
  }
  if (!readablePatch(patchText)) return deny(call.tool, "has unreadable patch headers");
  return request(call, ctx, "apply_patch", { command: patchText, patchText });
}

function task(call: ToolCall, args: Args, ctx: PermissionContext): PermissionMapping {
  const description = stringField(args, "description");
  const prompt = stringField(args, "prompt");
  const subagent = stringField(args, "subagent_type");
  if (
    description === undefined ||
    description.length === 0 ||
    prompt === undefined ||
    prompt.length === 0 ||
    subagent === undefined ||
    subagent.length === 0
  ) {
    return deny(call.tool, "missing string description, prompt and subagent_type");
  }
  if (presentWrong(args, "task_id", typeof args.task_id === "string")) {
    return deny(call.tool, "task_id must be a string");
  }
  if (presentWrong(args, "command", typeof args.command === "string")) {
    return deny(call.tool, "command must be a string");
  }
  if (presentWrong(args, "background", typeof args.background === "boolean")) {
    return deny(call.tool, "background must be a boolean");
  }
  if (presentWrong(args, "model", typeof args.model === "string")) {
    return deny(call.tool, "model must be a string");
  }
  if (presentWrong(args, "reasoning_effort", typeof args.reasoning_effort === "string")) {
    return deny(call.tool, "reasoning_effort must be a string");
  }
  const input: Args = { description, prompt, subagent_type: subagent };
  if (typeof args.task_id === "string") input.task_id = args.task_id;
  if (typeof args.command === "string") input.command = args.command;
  if (typeof args.background === "boolean") input.background = args.background;
  if (typeof args.model === "string") input.model = args.model;
  if (typeof args.reasoning_effort === "string") input.reasoning_effort = args.reasoning_effort;
  return request(call, ctx, "Task", input);
}

function mcp(
  call: ToolCall,
  args: unknown,
  ctx: PermissionContext,
  toolName: string,
): PermissionMapping {
  const record = argsOf(args);
  if (record === undefined) return deny(call.tool, "missing object args");
  return request(call, ctx, toolName, { ...record, opencode_tool: call.tool });
}

function known(call: ToolCall, args: Args, ctx: PermissionContext): PermissionMapping | undefined {
  switch (call.tool) {
    case "bash":
      return bash(call, args, ctx);
    case "edit":
      return edit(call, args, ctx);
    case "write":
      return write(call, args, ctx);
    case "read":
      return read(call, args, ctx);
    case "grep":
      return search(call, args, ctx, "Grep");
    case "glob":
      return search(call, args, ctx, "Glob");
    case "apply_patch":
      return patch(call, args, ctx);
    case "task":
      return task(call, args, ctx);
    default:
      return undefined;
  }
}

/** Map one host tool call (its validated `args`) to the core dispatcher's hook input. */
export function mapToolCall(
  call: ToolCall,
  args: unknown,
  ctx: PermissionContext,
  mcpServers: readonly string[] = [],
): PermissionMapping {
  const record = argsOf(args);
  const mcpName = mcpToolName(call.tool, mcpServers);
  const mapped =
    record !== undefined
      ? known(call, record, ctx)
      : call.tool === "bash" ||
          call.tool === "edit" ||
          call.tool === "write" ||
          call.tool === "read" ||
          call.tool === "grep" ||
          call.tool === "glob" ||
          call.tool === "apply_patch" ||
          call.tool === "task"
        ? deny(call.tool, "missing object args")
        : undefined;
  const resolved =
    mapped ?? (mcpName === undefined ? { kind: "skip" as const } : mcp(call, args, ctx, mcpName));
  if (resolved.kind !== "skip" && (call.sessionID === "" || call.callID === "")) {
    return deny(call.tool, "missing sessionID or callID");
  }
  return resolved;
}

/** MCP client names from `<projectRoot>/opencode.json`. Unreadable config is an empty list. */
export function mcpServerNames(projectRoot: string): string[] {
  try {
    const parsed: unknown = JSON.parse(readFileSync(join(projectRoot, "opencode.json"), "utf8"));
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return [];
    if (!("mcp" in parsed)) return [];
    const mcpConfig: unknown = parsed.mcp;
    if (typeof mcpConfig !== "object" || mcpConfig === null || Array.isArray(mcpConfig)) return [];
    return Object.keys(mcpConfig);
  } catch {
    return [];
  }
}

/** Refuse every tool call with `reason`: the fail-closed hook when toolu is not ready. */
export function createDenyAllToolBefore(reason: string): ToolBefore {
  return () => Promise.reject(new Error(reason));
}

/** Run the native gates for each covered tool call; preserve native permission checks. */
export function createToolBeforeHandler(
  opts: PermissionEvaluateHandlerOptions,
  advice?: ToolAdviceStore,
  onBegin?: (call: ToolCall) => void,
): ToolBefore {
  const decider = createGateDecider(opts);
  if (!decider.ok) return createDenyAllToolBefore(decider.reason);
  const servers = mcpServerNames(opts.permissionContext.projectRoot);
  return async (input, output) => {
    onBegin?.(input);
    advice?.begin(input);
    const args: unknown = output.args;
    const mapping = mapToolCall(input, args, opts.permissionContext, servers);
    if (mapping.kind === "skip") return;
    if (mapping.kind === "deny") throw new Error(mapping.reason);
    const decision = await decider.decide(mapping.request);
    switch (decision.kind) {
      case "allow":
        return;
      case "advisory":
        advice?.record(input, decision.message);
        return;
      case "deny":
      case "ask":
      case "post_block":
      case "runtime_failure":
        throw new Error(decision.reason);
    }
  };
}
