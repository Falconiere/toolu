import { expect, test } from "bun:test";
import { DecisionSchema, parseDecision } from "../decision.ts";

test("parseDecision accepts allow", () => {
  expect(parseDecision({ kind: "allow" })).toEqual({ kind: "allow" });
});

test("parseDecision accepts deny with reason", () => {
  expect(parseDecision({ kind: "deny", reason: "blocked" })).toEqual({
    kind: "deny",
    reason: "blocked",
  });
});

test("parseDecision rejects unknown kind", () => {
  expect(() => parseDecision({ kind: "maybe" })).toThrow();
});

test("parseDecision rejects deny without reason", () => {
  expect(() => parseDecision({ kind: "deny" })).toThrow();
});

test("DecisionSchema accepts runtime_failure codes", () => {
  const parsed = DecisionSchema.parse({
    kind: "runtime_failure",
    reason: "timeout",
    code: "timeout",
  });
  expect(parsed.kind === "runtime_failure" && parsed.code).toBe("timeout");
});
