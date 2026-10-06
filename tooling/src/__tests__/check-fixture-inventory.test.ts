import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import {
  checkCaptureFile,
  checkCommittedCaptures,
  checkInventory,
  checkSuite,
  readFixtureIndex,
} from "../check-fixture-inventory.ts";

test("the recorded case inventory has unique names and 1192 inputs", () => {
  const index = readFixtureIndex(resolve(import.meta.dir, "../../../fixtures/index.json"));
  expect(index.suites.reduce((count, suite) => count + suite.names.length, 0)).toBe(1192);
  expect(new Set(index.suites.map((suite) => suite.id)).size).toBe(index.suites.length);
});

test("committed captures cover their case records", () => {
  expect(() => checkCommittedCaptures(resolve(import.meta.dir, "../../.."))).not.toThrow();
});

test("capture audit rejects orphaned and missing captures", () => {
  using sb = createSandbox();
  const fixture = join(sb.project, "fixtures", "cases.json");
  const golden = join(sb.project, "fixtures", "golden.json");
  const pair = { fixture: "cases.json", golden: "golden.json", field: "cases" };
  mkdirSync(join(sb.project, "fixtures"));
  writeFileSync(fixture, '{"version":1,"cases":[{"name":"one"}]}');
  writeFileSync(golden, '{"cases":{"one":{},"orphan":{}}}');
  expect(() => checkCaptureFile(sb.project, pair)).toThrow("orphaned capture orphan");
  writeFileSync(golden, '{"cases":{}}');
  expect(() => checkCaptureFile(sb.project, pair)).toThrow("missing capture one");
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
