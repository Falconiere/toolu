import { expect, test } from "bun:test";
import { mergeDecisions, parseClassification } from "../policy.ts";

test("parseClassification accepts shell-out", () => {
  expect(parseClassification("shell-out")).toBe("shell-out");
});

test("mergeDecisions deny beats ask beats advisory beats allow", () => {
  expect(
    mergeDecisions([
      { kind: "allow" },
      { kind: "advisory", message: "note" },
      { kind: "ask", reason: "prompt" },
      { kind: "deny", reason: "no" },
    ]).kind,
  ).toBe("deny");

  expect(
    mergeDecisions([
      { kind: "allow" },
      { kind: "advisory", message: "note" },
      { kind: "ask", reason: "prompt" },
    ]).kind,
  ).toBe("ask");

  expect(mergeDecisions([{ kind: "allow" }, { kind: "advisory", message: "note" }]).kind).toBe(
    "advisory",
  );
});

test("mergeDecisions empty yields allow", () => {
  expect(mergeDecisions([])).toEqual({ kind: "allow" });
});
