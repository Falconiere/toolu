/** OpenCode permission.evaluate ↔ portable bridge (#204). */
import type { BridgeRequest } from "@toolu/core/bridge";
import type { Decision } from "@toolu/core/decision";

export type PermissionEffect = "allow" | "ask" | "deny";

/** OpenCode-shaped permission evaluation event (mutable effect for hook handlers). */
export type PermissionEvaluationEvent = {
  readonly sessionID: string;
  readonly agent?: string;
  readonly action: string;
  readonly resources: ReadonlyArray<string>;
  readonly metadata?: Record<string, unknown>;
  effect: PermissionEffect;
  message?: string;
};

export type PermissionBridgeContext = {
  cwd: string;
  projectRoot: string;
  worktree: string;
  host: string;
  deadlineMs?: number;
  maxStdoutBytes?: number;
};

export type PermissionBridgeMapping =
  | { kind: "skip" }
  | { kind: "request"; request: BridgeRequest }
  | { kind: "deny"; reason: string };

type GatedAction = {
  toolName: string;
  inputKind: "file" | "shell";
};

const GATED: Record<string, GatedAction> = {
  edit: { toolName: "Edit", inputKind: "file" },
  write: { toolName: "Write", inputKind: "file" },
  bash: { toolName: "Bash", inputKind: "shell" },
  shell: { toolName: "Shell", inputKind: "shell" },
};

const DEFAULT_DEADLINE_MS = 15_000;
const DEFAULT_MAX_STDOUT_BYTES = 1_048_576;

function normalizeAction(action: string): string {
  return action.trim().toLowerCase();
}

function readString(meta: Record<string, unknown> | undefined, key: string): string | undefined {
  if (!meta) {
    return undefined;
  }
  const value = meta[key];
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function filePathFromEvent(event: PermissionEvaluationEvent): string | undefined {
  const fromResource = event.resources[0];
  if (typeof fromResource === "string" && fromResource.length > 0) {
    return fromResource;
  }
  const meta = event.metadata;
  return readString(meta, "file_path") ?? readString(meta, "filePath") ?? readString(meta, "path");
}

function commandFromEvent(event: PermissionEvaluationEvent): string | undefined {
  const fromMeta = readString(event.metadata, "command");
  if (fromMeta) {
    return fromMeta;
  }
  const fromResource = event.resources[0];
  return typeof fromResource === "string" && fromResource.length > 0 ? fromResource : undefined;
}

function toolCallId(event: PermissionEvaluationEvent): string {
  const meta = event.metadata;
  const fromMeta = readString(meta, "toolCallId") ?? readString(meta, "tool_call_id");
  if (fromMeta) {
    return fromMeta;
  }
  const source = meta?.source;
  if (typeof source === "object" && source !== null && !Array.isArray(source)) {
    const sourceRecord: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(source)) {
      sourceRecord[key] = value;
    }
    const fromSource = readString(sourceRecord, "id");
    if (fromSource) {
      return fromSource;
    }
  }
  return `perm_${event.sessionID}`;
}

/** Map a permission.evaluate payload to a bridge tool/pre request, or skip/deny when incomplete. */
export function mapPermissionEventToBridge(
  event: PermissionEvaluationEvent,
  ctx: PermissionBridgeContext,
): PermissionBridgeMapping {
  const gated = GATED[normalizeAction(event.action)];
  if (!gated) {
    return { kind: "skip" };
  }

  let toolInput: Record<string, unknown>;
  if (gated.inputKind === "file") {
    const filePath = filePathFromEvent(event);
    if (!filePath) {
      return {
        kind: "deny",
        reason: `toolu: ${event.action} permission missing file path (resources or metadata)`,
      };
    }
    toolInput = { file_path: filePath };
  } else {
    const command = commandFromEvent(event);
    if (!command) {
      return {
        kind: "deny",
        reason: `toolu: ${event.action} permission missing shell command (metadata.command or resources)`,
      };
    }
    toolInput = { command };
  }

  const request: BridgeRequest = {
    protocolVersion: 1,
    event: "tool/pre",
    sessionId: event.sessionID,
    toolCallId: toolCallId(event),
    cwd: ctx.cwd,
    projectRoot: ctx.projectRoot,
    worktree: ctx.worktree,
    toolName: gated.toolName,
    toolInput,
    host: ctx.host,
    deadlineMs: ctx.deadlineMs ?? DEFAULT_DEADLINE_MS,
    maxStdoutBytes: ctx.maxStdoutBytes ?? DEFAULT_MAX_STDOUT_BYTES,
  };

  return { kind: "request", request };
}

/** Apply a portable Decision onto an OpenCode permission evaluation (mutates event). */
export function applyDecisionToPermission(
  decision: Decision,
  event: PermissionEvaluationEvent,
): void {
  switch (decision.kind) {
    case "deny":
      event.effect = "deny";
      event.message = decision.reason;
      return;
    case "ask":
      event.effect = "ask";
      event.message = decision.reason;
      return;
    case "allow":
      event.effect = "allow";
      delete event.message;
      return;
    case "advisory":
      event.effect = "allow";
      event.message = decision.message;
      return;
    case "post_block":
      event.effect = "deny";
      event.message = decision.reason;
      return;
    case "runtime_failure":
      event.effect = "deny";
      event.message = decision.reason;
      return;
  }
}
