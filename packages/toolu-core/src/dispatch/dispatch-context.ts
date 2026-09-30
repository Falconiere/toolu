/**
 * Module contract and per-payload context for the PreToolUse (#258) and
 * PostToolUse (#259) dispatchers. Built-in modules share `run(event, ctx)` with
 * #257's `RegistryModule`. Every one is native since #262 dropped the bash
 * fallback; every context comes from `toolContext`.
 */
import { isJsonObject } from "../config/config-load.ts";
import type { Decision } from "../decision/decision.ts";
import type { HostEnv, HostName } from "../host/host-name.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import type { EditRecord } from "../state/state-schema.ts";

/** Which hook a walk serves: `PreToolUse` or `PostToolUse`. */
export type HookPhase = "pre" | "post";

/** A built-in module: a native gate, run in-process. */
export type ToolModule = {
  readonly kind: "native";
  readonly name: string;
  run(event: RegistryHookEvent, ctx: RegistryContext): Promise<Decision>;
};

/** An edit split out of a multi-file patch: the `TOOLU_EDIT_*` a registry bash module sees. */
export type EditSplit = { operation: EditRecord["operation"]; from: string; movedTo: string };

/** One walk's inputs: the payload text every module reads, plus what it was built from. */
export type Payload = {
  readonly text: string;
  readonly toolName: string;
  readonly edit?: EditSplit;
};

/** What stays fixed across every walk of one hook call. */
export type Session = {
  readonly phase: HookPhase;
  readonly host: HostName;
  readonly env: HostEnv;
  readonly configRoot: string;
  readonly projectRoot: string;
  readonly libDir: string;
  /** The hook process's working directory. */
  readonly cwd: string;
};

function text(value: unknown, fallback: string): string {
  return typeof value === "string" && value !== "" ? value : fallback;
}

/**
 * The normalized event for `payload`. Post is `tool/post`, carrying the tool's
 * response; pre is `shell/pre` for a Bash/Shell call with a command, else `tool/pre`.
 */
export function toolEvent(payload: Payload, doc: unknown, session: Session): RegistryHookEvent {
  const raw = isJsonObject(doc) ? doc : {};
  const input = isJsonObject(raw.tool_input) ? raw.tool_input : {};
  const cwd = text(raw.cwd, session.projectRoot);
  const base = {
    sessionId: text(raw.session_id, "unknown"),
    cwd,
    projectRoot: session.projectRoot,
    worktree: session.projectRoot,
    toolCallId: text(raw.tool_use_id, "unknown"),
    toolName: text(payload.toolName, "unknown"),
    toolInput: input,
  };
  if (session.phase === "post") {
    const output = raw.tool_response ?? raw.tool_output;
    return { ...base, type: "tool/post", ...(output === undefined ? {} : { toolOutput: output }) };
  }
  const command = input.command;
  if (
    (payload.toolName === "Bash" || payload.toolName === "Shell") &&
    typeof command === "string" &&
    command !== ""
  ) {
    return { ...base, type: "shell/pre", command };
  }
  return { ...base, type: "tool/pre" };
}

/** The context a module sees for `payload`. */
export function toolContext(payload: Payload, doc: unknown, session: Session): RegistryContext {
  const edit = payload.edit;
  return {
    host: session.host,
    env: session.env,
    configRoot: session.configRoot,
    projectRoot: session.projectRoot,
    cwd: session.cwd,
    raw: isJsonObject(doc) ? doc : {},
    ...(edit === undefined ? {} : { edit }),
  };
}
