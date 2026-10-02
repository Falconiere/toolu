/** Deliver one pre-tool advisory through the pinned host's model-visible after output. */
import type { Hooks } from "@opencode-ai/plugin";
import type { ToolCall } from "./tool-before.ts";

type ToolAfter = NonNullable<Hooks["tool.execute.after"]>;
type Pending = { tool: string; message: string; expiresAt: number };

export type ToolAdviceStore = {
  begin: (call: ToolCall) => void;
  record: (call: ToolCall, message: string) => void;
  after: ToolAfter;
  clear: () => void;
};

const DEFAULT_TTL_MS = 5 * 60_000;
const MAX_PENDING = 256;
const MAX_MESSAGE_CHARS = 8192;

function key(call: ToolCall): string {
  return JSON.stringify([call.sessionID, call.callID]);
}

/** A plugin instance owns this store; no approval or permission state is cached. */
export function createToolAdviceStore(options: { ttlMs?: number } = {}): ToolAdviceStore {
  const pending = new Map<string, Pending>();
  const ttlMs = options.ttlMs ?? DEFAULT_TTL_MS;

  function prune(): void {
    const now = Date.now();
    for (const [id, entry] of pending) {
      if (entry.expiresAt <= now) pending.delete(id);
    }
  }

  const begin = (call: ToolCall): void => {
    prune();
    pending.delete(key(call));
  };

  const record = (call: ToolCall, message: string): void => {
    prune();
    const id = key(call);
    pending.delete(id);
    if (pending.size >= MAX_PENDING) {
      const oldest = pending.keys().next().value;
      if (oldest !== undefined) pending.delete(oldest);
    }
    const bounded =
      message.length > MAX_MESSAGE_CHARS
        ? `${message.slice(0, MAX_MESSAGE_CHARS)}\n[toolu advisory truncated]`
        : message;
    pending.set(id, { tool: call.tool, message: bounded, expiresAt: Date.now() + ttlMs });
  };

  const after: ToolAfter = (input, output) => {
    prune();
    const id = key(input);
    const entry = pending.get(id);
    pending.delete(id);
    if (entry === undefined || entry.tool !== input.tool) return Promise.resolve();
    if (typeof output.output !== "string") {
      return Promise.reject(
        new Error("toolu: advisory delivery failed after tool execution: output is not text"),
      );
    }
    output.output = `${output.output}\n\n[toolu advisory]\n${entry.message}`;
    return Promise.resolve();
  };

  return { begin, record, after, clear: () => pending.clear() };
}
