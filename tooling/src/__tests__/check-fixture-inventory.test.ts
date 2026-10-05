import { expect, test } from "bun:test";
import { writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { checkInventory, checkSuite, readFixtureIndex } from "../check-fixture-inventory.ts";

test("the recorded exported-case inventory has unique names and 979 inputs", () => {
  const index = readFixtureIndex(resolve(import.meta.dir, "../../../fixtures/index.json"));
  expect(index.suites.reduce((count, suite) => count + suite.names.length, 0)).toBe(979);
  expect(new Set(index.suites.map((suite) => suite.id)).size).toBe(index.suites.length);
});

test("shared case files preserve each suite and cover all names exactly once", () => {
  using sb = createSandbox();
  const path = join(sb.project, "cases.json");
  const suites = [
    { id: "a", fixture: "cases.json", source: "a.ts", namePrefix: "a:", names: ["a: one"] },
    { id: "b", fixture: "cases.json", source: "b.ts", namePrefix: "b:", names: ["b: one"] },
  ];
  writeFileSync(path, '{"version":1,"cases":[{"name":"a: one"},{"name":"b: one"}]}');
  expect(() => checkInventory(sb.project, { version: 1, suites })).not.toThrow();
  writeFileSync(
    path,
    '{"version":1,"cases":[{"name":"a: one"},{"name":"b: one"},{"name":"c: extra"}]}',
  );
  expect(() => checkInventory(sb.project, { version: 1, suites })).toThrow("exactly once");
});

test("a committed case file must keep the original names, including when counts match", () => {
  using sb = createSandbox();
  const path = join(sb.project, "cases.json");
  const suite = { id: "example", fixture: "cases.json", source: "old-cases.ts", names: ["a", "b"] };
  writeFileSync(path, '{"version":1,"cases":[{"name":"a"},{"name":"b"}]}');
  expect(() => checkSuite(sb.project, suite)).not.toThrow();
  writeFileSync(path, '{"version":1,"cases":[{"name":"a"},{"name":"c"}]}');
  expect(() => checkSuite(sb.project, suite)).toThrow("case names differ");
});
