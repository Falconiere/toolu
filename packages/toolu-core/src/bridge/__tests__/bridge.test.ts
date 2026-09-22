import { expect, test } from "bun:test";
import { decisionFromHookResult, parseBridgeRequest, toHookStdin } from "../bridge.ts";

test("parseBridgeRequest accepts protected-files fixture shape", () => {
  const req = parseBridgeRequest({
    protocolVersion: 1,
    event: "tool/pre",
    sessionId: "s",
    toolCallId: "c",
    cwd: "/repo",
    projectRoot: "/repo",
    worktree: "/repo",
    toolName: "Edit",
    toolInput: { file_path: "/repo/.env" },
    host: "opencode",
    deadlineMs: 15000,
    maxStdoutBytes: 1048576,
  });
  expect(req.toolName).toBe("Edit");
});

test("parseBridgeRequest rejects bad protocolVersion", () => {
  expect(() =>
    parseBridgeRequest({
      protocolVersion: 2,
      event: "tool/pre",
      sessionId: "s",
      toolCallId: "c",
      cwd: "/r",
      projectRoot: "/r",
      worktree: "/r",
      toolName: "Edit",
      host: "x",
      deadlineMs: 1,
      maxStdoutBytes: 1,
    }),
  ).toThrow();
});

test("toHookStdin maps Claude PreToolUse fields", () => {
  expect(toHookStdin({ toolName: "Edit", toolInput: { file_path: "/a" } })).toBe(
    JSON.stringify({ tool_name: "Edit", tool_input: { file_path: "/a" } }),
  );
});

test("decisionFromHookResult exit 2 pre yields deny with stderr reason", () => {
  expect(decisionFromHookResult(2, "", "protected file", "tool/pre")).toEqual({
    kind: "deny",
    reason: "protected file",
  });
});

test("decisionFromHookResult exit 2 post yields post_block", () => {
  expect(decisionFromHookResult(2, "", "blocked", "tool/post")).toEqual({
    kind: "post_block",
    reason: "blocked",
  });
});

test("decisionFromHookResult exit 0 empty stdout yields allow", () => {
  expect(decisionFromHookResult(0, "", "", "tool/pre")).toEqual({ kind: "allow" });
});

test("decisionFromHookResult permissionDecision deny", () => {
  const stdout = JSON.stringify({
    hookSpecificOutput: {
      permissionDecision: "deny",
      permissionDecisionReason: "no",
    },
  });
  expect(decisionFromHookResult(0, stdout, "", "tool/pre")).toEqual({
    kind: "deny",
    reason: "no",
  });
});

test("decisionFromHookResult malformed stdout yields runtime_failure parse", () => {
  expect(decisionFromHookResult(0, "not-json", "", "tool/pre")).toEqual({
    kind: "runtime_failure",
    reason: "hook stdout was not valid JSON",
    code: "parse",
  });
});
