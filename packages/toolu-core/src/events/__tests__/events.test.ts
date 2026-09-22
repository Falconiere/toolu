import { expect, test } from "bun:test";
import { parseNormalizedEvent } from "../events.ts";

const base = {
  sessionId: "s1",
  cwd: "/repo",
  projectRoot: "/repo",
  worktree: "/repo",
};

test("parseNormalizedEvent accepts tool/pre", () => {
  expect(
    parseNormalizedEvent({
      type: "tool/pre",
      ...base,
      toolCallId: "c1",
      toolName: "Edit",
      toolInput: { file_path: "/repo/a.ts" },
    }).type,
  ).toBe("tool/pre");
});

test("parseNormalizedEvent rejects missing sessionId", () => {
  expect(() =>
    parseNormalizedEvent({
      type: "tool/pre",
      cwd: "/repo",
      projectRoot: "/repo",
      worktree: "/repo",
      toolCallId: "c1",
      toolName: "Edit",
    }),
  ).toThrow();
});

test("parseNormalizedEvent accepts session/start", () => {
  expect(parseNormalizedEvent({ type: "session/start", ...base }).type).toBe("session/start");
});
