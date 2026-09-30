import { expect, test } from "bun:test";
import { decisionFromDispatch } from "../result.ts";

test("unknown dispatcher decisions fail closed", () => {
  const result = decisionFromDispatch({
    exitCode: 0,
    stderr: "",
    stdout: JSON.stringify({ hookSpecificOutput: { permissionDecision: "later" } }),
  });
  expect(result).toMatchObject({ kind: "runtime_failure", code: "parse" });
});

test("an empty JSON object is not a silent allow", () => {
  const result = decisionFromDispatch({ exitCode: 0, stderr: "", stdout: "{}" });
  expect(result).toMatchObject({ kind: "runtime_failure", code: "parse" });
});

test("silent dispatcher success allows and explicit deny blocks", () => {
  expect(decisionFromDispatch({ exitCode: 0, stderr: "", stdout: "" })).toEqual({ kind: "allow" });
  expect(
    decisionFromDispatch({
      exitCode: 0,
      stderr: "",
      stdout: JSON.stringify({
        hookSpecificOutput: { permissionDecision: "deny", permissionDecisionReason: "protected" },
      }),
    }),
  ).toEqual({ kind: "deny", reason: "protected" });
});
