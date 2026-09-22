/** Map hook stdout/exit codes to portable decisions (#210). */
import { type Decision } from "../decision/decision.ts";
import { type BridgeEvent } from "../events/events.ts";

type HookJson = {
  hookSpecificOutput?: {
    permissionDecision?: string;
    permissionDecisionReason?: string;
    additionalContext?: string;
  };
  decision?: string;
  reason?: string;
  systemMessage?: string;
};

function readStringField(obj: Record<string, unknown>, key: string): string | undefined {
  const v = obj[key];
  return typeof v === "string" ? v : undefined;
}

function objectToRecord(value: object): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [key, val] of Object.entries(value)) {
    out[key] = val;
  }
  return out;
}

function parseHookObject(parsed: Record<string, unknown>): HookJson {
  const out: HookJson = {};
  const hso = parsed.hookSpecificOutput;
  if (typeof hso === "object" && hso !== null && !Array.isArray(hso)) {
    const h = objectToRecord(hso);
    const hookOut: NonNullable<HookJson["hookSpecificOutput"]> = {};
    const permissionDecision = readStringField(h, "permissionDecision");
    const permissionDecisionReason = readStringField(h, "permissionDecisionReason");
    const additionalContext = readStringField(h, "additionalContext");
    if (permissionDecision !== undefined) {
      hookOut.permissionDecision = permissionDecision;
    }
    if (permissionDecisionReason !== undefined) {
      hookOut.permissionDecisionReason = permissionDecisionReason;
    }
    if (additionalContext !== undefined) {
      hookOut.additionalContext = additionalContext;
    }
    out.hookSpecificOutput = hookOut;
  }
  const decision = readStringField(parsed, "decision");
  const reason = readStringField(parsed, "reason");
  const systemMessage = readStringField(parsed, "systemMessage");
  if (decision !== undefined) {
    out.decision = decision;
  }
  if (reason !== undefined) {
    out.reason = reason;
  }
  if (systemMessage !== undefined) {
    out.systemMessage = systemMessage;
  }
  return out;
}

function parseHookStdout(stdout: string): HookJson | null {
  const trimmed = stdout.trim();
  if (trimmed === "") {
    return null;
  }
  try {
    const parsed: unknown = JSON.parse(trimmed);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      return null;
    }
    return parseHookObject(objectToRecord(parsed));
  } catch {
    return null;
  }
}

function runtimeFailure(
  reason: string,
  code: Extract<Decision, { kind: "runtime_failure" }>["code"],
): Decision {
  return { kind: "runtime_failure", reason, code };
}

function isPostEvent(event: BridgeEvent): boolean {
  return event === "tool/post";
}

/** Claude PreToolUse / PostToolUse stdin JSON from a bridge request. */
export function toHookStdin(request: {
  toolName: string;
  toolInput: Record<string, unknown>;
}): string {
  return JSON.stringify({
    tool_name: request.toolName,
    tool_input: request.toolInput,
  });
}

/** Normalize bash dispatch stdout + exit code into a Decision. */
export function decisionFromHookResult(
  exitCode: number,
  stdout: string,
  stderr: string,
  event: BridgeEvent,
): Decision {
  if (exitCode === 2) {
    const reason = stderr.trim() || "hook blocked via exit code 2";
    if (isPostEvent(event)) {
      return { kind: "post_block", reason };
    }
    return { kind: "deny", reason };
  }

  if (exitCode !== 0) {
    return runtimeFailure(stderr.trim() || `hook exited ${exitCode}`, "nonzero");
  }

  const payload = parseHookStdout(stdout);
  if (stdout.trim() !== "" && payload === null) {
    return runtimeFailure("hook stdout was not valid JSON", "parse");
  }

  if (payload === null) {
    return { kind: "allow" };
  }

  const permission = payload.hookSpecificOutput?.permissionDecision;
  if (permission === "deny") {
    const fromHook = payload.hookSpecificOutput?.permissionDecisionReason?.trim();
    const fromReason = payload.reason?.trim();
    const reason =
      fromHook && fromHook.length > 0
        ? fromHook
        : fromReason && fromReason.length > 0
          ? fromReason
          : "permission denied";
    return { kind: "deny", reason };
  }
  if (permission === "ask") {
    const fromHook = payload.hookSpecificOutput?.permissionDecisionReason?.trim();
    const reason = fromHook && fromHook.length > 0 ? fromHook : "permission ask";
    return { kind: "ask", reason };
  }

  if (isPostEvent(event) && payload.decision === "block") {
    const fromReason = payload.reason?.trim();
    const reason = fromReason && fromReason.length > 0 ? fromReason : "post-tool block";
    return { kind: "post_block", reason };
  }

  const ctx = payload.hookSpecificOutput?.additionalContext?.trim();
  const sys = payload.systemMessage?.trim();
  const message = [ctx, sys].filter((part) => part && part.length > 0).join("\n\n");
  if (message.length > 0) {
    return { kind: "advisory", message };
  }

  return { kind: "allow" };
}

export function decisionFromRunnerFailure(code: string, message: string): Decision {
  if (code === "timeout") {
    return runtimeFailure(message, "timeout");
  }
  if (code === "spawn") {
    return runtimeFailure(message, "spawn");
  }
  if (code === "truncated") {
    return runtimeFailure(message, "truncated");
  }
  if (code === "cancelled") {
    return runtimeFailure(message, "cancelled");
  }
  return runtimeFailure(message, "nonzero");
}
