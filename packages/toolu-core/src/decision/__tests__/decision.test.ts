import { expect, test } from "bun:test";
import { parseDecision } from "../decision.ts";

test("parseDecision accepts allow", () => {
  expect(parseDecision({ kind: "allow" })).toEqual({ kind: "allow" });
});

test("parseDecision rejects unknown kind", () => {
  expect(() => parseDecision({ kind: "maybe" })).toThrow();
});
