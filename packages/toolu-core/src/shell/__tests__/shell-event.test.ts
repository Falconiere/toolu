/**
 * One parse per `shell/pre` event (#284 AC-9): every module asking about the
 * same event gets the same analysis object, and another event gets its own.
 */
import { expect, test } from "bun:test";
import { parseNormalizedEvent } from "../../events/events.ts";
import { shellAnalysisOf, type ShellPreEvent } from "../shell-event.ts";

function shellEvent(command: string): ShellPreEvent {
  const event = parseNormalizedEvent({
    type: "shell/pre",
    sessionId: "s1",
    cwd: "/repo",
    projectRoot: "/repo",
    worktree: "/repo",
    toolCallId: "call-1",
    toolName: "Bash",
    command,
  });
  if (event.type !== "shell/pre") throw new Error(`expected shell/pre, got ${event.type}`);
  return event;
}

test.concurrent("repeated calls on one event share one analysis", () => {
  const event = shellEvent("git push origin HEAD:feat/x");
  const first = shellAnalysisOf(event);
  expect(first.source).toBe(event.command);
  expect(first.commands.map((c) => c.argv)).toEqual([["git", "push", "origin", "HEAD:feat/x"]]);
  expect(shellAnalysisOf(event)).toBe(first);
});

test.concurrent("a different event gets its own analysis, even for the same command", () => {
  const a = shellEvent("ls");
  const b = shellEvent("ls");
  expect(shellAnalysisOf(a)).not.toBe(shellAnalysisOf(b));
  expect(shellAnalysisOf(a)).toEqual(shellAnalysisOf(b));
});
