/**
 * The shared analysis fixture (#416): `fixtures/shell/analysis.json` holds the
 * projected analysis of every input of `unbash-baseline.json`, in the same
 * order, and TypeScript reproduces every `expect`. The Rust port
 * (`crates/core/shell/tests/analysis_fixture.rs`) reproduces `expect`, or
 * `rust.expect` where `rust.reason` records an intended difference; every
 * such reason is listed in docs/shell-analysis.md.
 */
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { projectAnalysis } from "./analysis-projection.ts";
import { FIXTURES, REPO } from "./parity-helpers.ts";

const Fixture = z.strictObject({
  version: z.literal(1),
  parser: z.literal("unbash@4.0.11"),
  cases: z.array(
    z.strictObject({
      input: z.string(),
      expect: z.json(),
      rust: z.strictObject({ expect: z.json(), reason: z.string().min(1) }).optional(),
    }),
  ),
});
const Baseline = z.looseObject({ cases: z.array(z.looseObject({ input: z.string() })) });

const read = (name: string): unknown => JSON.parse(readFileSync(join(FIXTURES, name), "utf8"));
const fixture = Fixture.parse(read("analysis.json"));

test("the fixture covers exactly the unbash baseline inputs, in order", () => {
  const baseline = Baseline.parse(read("unbash-baseline.json"));
  expect(fixture.cases.map((c) => c.input)).toEqual(baseline.cases.map((c) => c.input));
});

test("TypeScript reproduces every projected analysis", () => {
  const differing = fixture.cases.filter(
    (c) => !Bun.deepEquals(projectAnalysis(c.input), c.expect),
  );
  expect(differing.map((c) => c.input)).toEqual([]);
});

test("every intended Rust difference is listed with its reason in docs/shell-analysis.md", () => {
  const docs = readFileSync(join(REPO, "docs/shell-analysis.md"), "utf8");
  const reasons = fixture.cases.flatMap((c) => (c.rust === undefined ? [] : [c.rust.reason]));
  expect(reasons.filter((reason) => !docs.includes(reason))).toEqual([]);
});
