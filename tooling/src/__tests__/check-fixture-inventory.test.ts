import { expect, test } from "bun:test";
import { writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { checkSuite, readFixtureIndex } from "../check-fixture-inventory.ts";

test("the recorded exported-case inventory has unique names and 979 inputs", () => {
  const index = readFixtureIndex(resolve(import.meta.dir, "../../../fixtures/index.json"));
  expect(index.suites.reduce((count, suite) => count + suite.names.length, 0)).toBe(979);
  expect(new Set(index.suites.map((suite) => suite.id)).size).toBe(index.suites.length);
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
