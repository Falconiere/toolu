/**
 * SessionStart context output (#269): the `hookSpecificOutput` shape Claude
 * Code and Codex both accept, bounded to the length a host injects, printed
 * with jq's bytes so ports stay byte-identical to the bash hooks.
 */
import { toJqJson } from "../state/state-io.ts";

/** Claude Code's ceiling on hook `additionalContext`; longer text is cut, never rejected. */
export const MAX_CONTEXT_CHARS = 10_000;

export type SessionContext = {
  hookSpecificOutput: { hookEventName: string; additionalContext: string };
};

/** `text` cut to `max` UTF-16 units, never splitting a surrogate pair. */
function bounded(text: string, max: number): string {
  if (text.length <= max) return text;
  const code = text.charCodeAt(max - 1);
  const end = code >= 0xd800 && code <= 0xdbff ? max - 1 : max;
  return text.slice(0, end);
}

/** The context payload for `event`, or `undefined` when there is nothing to say. */
export function sessionContext(event: string, text: string): SessionContext | undefined {
  if (text === "") return undefined;
  return {
    hookSpecificOutput: {
      hookEventName: event,
      additionalContext: bounded(text, MAX_CONTEXT_CHARS),
    },
  };
}

/** `value` as `jq -n` (pretty) or `jq -nc` (compact) prints it, newline-terminated. */
export function renderHookOutput(value: unknown, pretty: boolean): string {
  return `${toJqJson(value, pretty)}\n`;
}
