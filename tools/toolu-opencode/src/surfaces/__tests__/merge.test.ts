import { expect, test } from "bun:test";
import { isPlainRecord, mergeUnder } from "../merge.ts";

function rec(value: unknown): Record<string, unknown> {
  if (!isPlainRecord(value)) throw new Error(`not a record: ${String(value)}`);
  return value;
}

test("the user's keys win and nested records merge, base keys first", () => {
  const merged = mergeUnder(
    { description: "toolu", permission: { "*": "deny", read: "allow" }, mode: "subagent" },
    { description: "mine", permission: { read: "deny", webfetch: "allow" } },
  );
  expect(merged).toEqual({
    description: "mine",
    permission: { "*": "deny", read: "deny", webfetch: "allow" },
    mode: "subagent",
  });
  expect(Object.keys(rec(merged.permission))).toEqual(["*", "read", "webfetch"]);
});

test("recursion follows only toolu's own records, so a cyclic user value terminates", () => {
  const cyclic: Record<string, unknown> = { read: "deny" };
  cyclic.self = cyclic;
  const over: Record<string, unknown> = { permission: cyclic };
  over.loop = over;
  const merged = mergeUnder({ permission: { "*": "deny" } }, over);
  const permission = merged.permission;
  expect(permission).toMatchObject({ "*": "deny", read: "deny" });
  expect(rec(permission).self).toBe(cyclic);
  expect(merged.loop).toBe(over);
});
