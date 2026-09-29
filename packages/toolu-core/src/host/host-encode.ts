/**
 * Per-host output encoders (#252): one `Decision` in, the JSON each host's hook
 * contract accepts out. Normalisation runs first and fails closed, so a caller
 * mistake (an undegraded `ask`, a runtime failure) never becomes a silent allow.
 *
 * `ask` degradation follows `gate-mode.sh`: where the host cannot prompt, a
 * security guardrail's ask becomes a block and a judgement gate's becomes advice.
 */
import type { Decision } from "../decision/decision.ts";
import { nativeEventName, type HostEvent } from "./host-events.ts";
import type { HostName } from "./host-name.ts";

export type GateClass = "guardrail" | "judgement";

export type EncodedOutput =
  | { kind: "command"; stdout: string; stderr: string; exitCode: 0 }
  | { kind: "effect"; effect: "allow" | "deny" | "ask"; message?: string };

/** A decision after normalisation: no runtime failures, and only what the event can carry. */
type Normalized = Exclude<Decision, { kind: "runtime_failure" }>;

const PRE_ACTION: ReadonlySet<HostEvent> = new Set(["tool/pre", "shell/pre"]);
const BLOCKING: ReadonlySet<HostEvent> = new Set(["tool/pre", "shell/pre", "permission/evaluate"]);
const CONTEXT_ONLY: ReadonlySet<HostEvent> = new Set([
  "session/start",
  "session/unload",
  "pre_compact",
]);
/** Claude/Codex events whose hookSpecificOutput carries additionalContext. */
const ADDITIONAL_CONTEXT: ReadonlySet<HostEvent> = new Set([
  "tool/pre",
  "shell/pre",
  "tool/post",
  "session/start",
  "prompt",
]);

const ASK_EVENTS: Readonly<Record<HostName, ReadonlySet<HostEvent>>> = {
  claude: new Set(["tool/pre", "shell/pre", "permission/evaluate"]),
  // Codex drops PreToolUse ask and runs the tool; PermissionRequest shows its own prompt.
  codex: new Set(["permission/evaluate"]),
  // Cursor accepts but does not enforce ask on preToolUse.
  cursor: new Set(["shell/pre"]),
  hermes: new Set(),
  opencode: new Set(["tool/pre", "shell/pre", "permission/evaluate"]),
};

/** Can `host` put an `ask` for `event` in front of a human? Default event: PreToolUse. */
export function supportsAsk(host: HostName, event: HostEvent = "tool/pre"): boolean {
  return ASK_EVENTS[host].has(event);
}

/** Degrade `ask` where the host cannot prompt: guardrail → deny, judgement → advisory. */
export function degradeAsk(
  host: HostName,
  event: HostEvent,
  decision: Decision,
  gateClass: GateClass,
): Decision {
  if (decision.kind !== "ask" || supportsAsk(host, event)) {
    return decision;
  }
  return gateClass === "guardrail"
    ? { kind: "deny", reason: decision.reason }
    : { kind: "advisory", message: decision.reason };
}

function normalize(host: HostName, event: HostEvent, decision: Decision): Normalized {
  if (decision.kind === "runtime_failure") {
    return BLOCKING.has(event)
      ? { kind: "deny", reason: decision.reason }
      : { kind: "advisory", message: decision.reason };
  }
  if (decision.kind === "ask" && !supportsAsk(host, event)) {
    return PRE_ACTION.has(event)
      ? { kind: "deny", reason: decision.reason }
      : { kind: "advisory", message: decision.reason };
  }
  if (decision.kind === "deny" || decision.kind === "post_block") {
    if (event === "tool/post") return { kind: "post_block", reason: decision.reason };
    if (CONTEXT_ONLY.has(event)) return { kind: "advisory", message: decision.reason };
    return { kind: "deny", reason: decision.reason };
  }
  return decision;
}

function text(decision: Normalized): string {
  return decision.kind === "advisory"
    ? decision.message
    : decision.kind === "allow"
      ? ""
      : decision.reason;
}

function json(value: object): string {
  return `${JSON.stringify(value)}\n`;
}

/** Claude Code and Codex: `hookSpecificOutput`, silent success on allow. */
function hookOutput(native: string, event: HostEvent, decision: Normalized): string {
  if (decision.kind === "allow") return "";
  if (decision.kind === "post_block") return json({ decision: "block", reason: decision.reason });
  if (decision.kind === "advisory") {
    return ADDITIONAL_CONTEXT.has(event)
      ? json({ hookSpecificOutput: { hookEventName: native, additionalContext: decision.message } })
      : json({ systemMessage: decision.message });
  }
  if (event === "permission/evaluate") {
    return decision.kind === "deny"
      ? json({
          hookSpecificOutput: {
            hookEventName: native,
            decision: { behavior: "deny", message: decision.reason },
          },
        })
      : "";
  }
  if (event === "prompt") return json({ decision: "block", reason: decision.reason });
  return json({
    hookSpecificOutput: {
      hookEventName: native,
      permissionDecision: decision.kind,
      permissionDecisionReason: decision.reason,
    },
  });
}

/** Cursor: permission events always name a `permission`; an empty reply blocks. */
function cursorOutput(event: HostEvent, decision: Normalized): string {
  if (PRE_ACTION.has(event)) {
    if (decision.kind === "deny" || decision.kind === "ask") {
      return json({
        permission: decision.kind,
        user_message: decision.reason,
        agent_message: decision.reason,
      });
    }
    return decision.kind === "advisory"
      ? json({ permission: "allow", agent_message: decision.message })
      : json({ permission: "allow" });
  }
  if (event === "prompt") {
    return decision.kind === "deny"
      ? json({ continue: false, user_message: decision.reason })
      : json({ continue: true });
  }
  const hasContext = decision.kind === "advisory" || decision.kind === "post_block";
  if (hasContext && (event === "tool/post" || event === "session/start")) {
    return json({ additional_context: text(decision) });
  }
  return json({});
}

/** Hermes: only pre_tool_call blocks and only pre_llm_call takes context. */
function hermesOutput(event: HostEvent, decision: Normalized): string {
  if (PRE_ACTION.has(event) && decision.kind === "deny") {
    return json({ action: "block", message: decision.reason });
  }
  if (event === "prompt" && (decision.kind === "advisory" || decision.kind === "deny")) {
    return json({ context: text(decision) });
  }
  return "";
}

/** OpenCode runs in process: the permission effect it mutates. */
function opencodeEffect(decision: Normalized): EncodedOutput {
  if (decision.kind === "deny" || decision.kind === "ask") {
    return { kind: "effect", effect: decision.kind, message: decision.reason };
  }
  if (decision.kind === "advisory" || decision.kind === "post_block") {
    return { kind: "effect", effect: "allow", message: text(decision) };
  }
  return { kind: "effect", effect: "allow" };
}

/** Encode `decision` for `event` in `host`'s native hook output. */
export function encodeDecision(
  host: HostName,
  event: HostEvent,
  decision: Decision,
): EncodedOutput {
  const native = nativeEventName(host, event);
  if (native === null) {
    throw new Error(`${host} has no native event for ${event}`);
  }
  const normalized = normalize(host, event, decision);
  if (host === "opencode") {
    return opencodeEffect(normalized);
  }
  const stdout =
    host === "cursor"
      ? cursorOutput(event, normalized)
      : host === "hermes"
        ? hermesOutput(event, normalized)
        : hookOutput(native, event, normalized);
  return { kind: "command", stdout, stderr: "", exitCode: 0 };
}
