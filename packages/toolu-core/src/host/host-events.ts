/**
 * Canonical ↔ native hook event names (#252). The canonical slugs are the
 * `@toolu/core/events` vocabulary; each host names them its own way: Claude and
 * Codex PascalCase, Cursor camelCase, Hermes snake_case, OpenCode dotted plugin
 * hooks. `null` marks an event the host does not have.
 *
 * `HOST_EVENTS` is the subset of `BridgeEvent` toolu registers hooks for.
 * `session/resume` and `session/clear` arrive as the host's session-start event
 * (a `source` matcher on Claude/Codex), and `compaction` has no hook on most
 * hosts, so none of the three has its own row.
 */
import type { BridgeEvent } from "../events/events.ts";
import { HOST_NAMES, type HostName } from "./host-name.ts";

export const HOST_EVENTS = [
  "session/start",
  "session/unload",
  "prompt",
  "pre_compact",
  "permission/evaluate",
  "tool/pre",
  "shell/pre",
  "tool/post",
] as const satisfies readonly BridgeEvent[];

export type HostEvent = (typeof HOST_EVENTS)[number];

type EventTable = Readonly<Record<HostEvent, string | null>>;

const PASCAL: EventTable = {
  "session/start": "SessionStart",
  "session/unload": "SessionEnd",
  prompt: "UserPromptSubmit",
  pre_compact: "PreCompact",
  "permission/evaluate": "PermissionRequest",
  "tool/pre": "PreToolUse",
  "shell/pre": "PreToolUse",
  "tool/post": "PostToolUse",
};

const TABLES: Readonly<Record<HostName, EventTable>> = {
  claude: PASCAL,
  codex: PASCAL,
  cursor: {
    "session/start": "sessionStart",
    "session/unload": "sessionEnd",
    prompt: "beforeSubmitPrompt",
    pre_compact: "preCompact",
    "permission/evaluate": null,
    "tool/pre": "preToolUse",
    "shell/pre": "beforeShellExecution",
    "tool/post": "postToolUse",
  },
  hermes: {
    "session/start": "on_session_start",
    "session/unload": "on_session_end",
    prompt: "pre_llm_call",
    pre_compact: null,
    "permission/evaluate": null,
    "tool/pre": "pre_tool_call",
    "shell/pre": "pre_tool_call",
    "tool/post": "post_tool_call",
  },
  opencode: {
    "session/start": "session.created",
    "session/unload": "session.deleted",
    prompt: "chat.message",
    pre_compact: "experimental.session.compacting",
    "permission/evaluate": "permission.evaluate",
    "tool/pre": "tool.execute.before",
    "shell/pre": "tool.execute.before",
    "tool/post": "tool.execute.after",
  },
};

/** Native names that only reverse-map (Cursor splits tool events by kind). */
const ALIASES: Readonly<Partial<Record<HostName, Readonly<Record<string, HostEvent>>>>> = {
  cursor: {
    beforeMCPExecution: "tool/pre",
    afterFileEdit: "tool/post",
    afterShellExecution: "tool/post",
    afterMCPExecution: "tool/post",
  },
};

/** The host's name for `event`, or `null` when the host has no such event. */
export function nativeEventName(host: HostName, event: HostEvent): string | null {
  return TABLES[host][event];
}

/** The canonical event for a host's native name; a name shared by two rows maps to the first. */
export function canonicalEvent(host: HostName, native: string): HostEvent | null {
  const found = HOST_EVENTS.find((event) => TABLES[host][event] === native);
  return found ?? ALIASES[host]?.[native] ?? null;
}

/** Every host whose table (or aliases) contains `native`. */
export function hostsForNativeEvent(native: string): HostName[] {
  return HOST_NAMES.filter((host) => canonicalEvent(host, native) !== null);
}
