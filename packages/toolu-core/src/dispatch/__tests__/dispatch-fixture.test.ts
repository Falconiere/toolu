/**
 * The shared dispatch fixture (#418): the TypeScript dispatcher reproduces every
 * case of `fixtures/dispatch/cases.json` byte for byte. The Rust engine
 * (`crates/core/engine/tests/dispatch_fixture.rs`) checks the same cases.
 */
import { expect, test } from "bun:test";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { CaseSchema, FIXTURE, expand, runCase } from "./dispatch-fixture-harness.ts";

const cases = readCaseFile(FIXTURE).map((raw) => CaseSchema.parse(raw));

test.concurrent.each(cases.map((c) => [c.name, c] as const))("%s", async (_name, c) => {
  const { result, paths } = await runCase(c);
  expect({ stdout: result.stdout, stderr: result.stderr, exitCode: result.exitCode }).toEqual({
    stdout: expand(c.expect.stdout, paths),
    stderr: expand(c.expect.stderr, paths),
    exitCode: c.expect.exitCode,
  });
});
