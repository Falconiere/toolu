/**
 * AC-1, AC-10 (#260): every case of the deleted protected-files, mcp-blocker and
 * code-edit-rules bats suites, plus boundary cases, run through the committed
 * bundles behind their hooks.json launchers. Each keeps its bats intent and
 * prints what bash printed at the base commit; a case with a `deviation` is
 * one bash got wrong (#283) or a TypeScript core contract (#253), and there the
 * capture must differ.
 */
import { describe, expect, test } from "bun:test";
import {
  entryImplementation,
  implementationTag,
  launchedArgv,
} from "@toolu/conformance/harness/entry-command";
import {
  decisionOf,
  MODULE_CASES,
  runCase,
  type Entry,
  type ModuleCase,
} from "./pre-tool-modules-a-cases.ts";
import { readGolden } from "./pre-tool-modules-a-golden.ts";
import { comparable } from "./pre-tool-modules-a-parity.ts";

const golden = readGolden();

/** Each case spawns git and Bun; 5 s is too tight on a loaded machine. */
const CASE_TIMEOUT_MS = 30_000;

function bundle(entry: Entry): string[] {
  return launchedArgv({ plugin: "toolu", event: "PreToolUse", entry });
}

function intent(c: ModuleCase, stdout: string): void {
  const { outcome, text } = decisionOf(stdout);
  expect(outcome).toBe(c.expect);
  for (const part of c.has ?? []) expect(text).toContain(part);
  for (const part of c.lacks ?? []) expect(text).not.toContain(part);
}

describe("pre-tool modules A", () => {
  test.concurrent.each(
    MODULE_CASES.map((c) => [`${c.name}${implementationTag("toolu", c.entry)}`, c] as const),
  )(
    "%s",
    async (_name, c) => {
      const want = golden.cases[c.name];
      if (want === undefined) throw new Error(`no golden capture for ${c.name}`);
      const got = await runCase(c, bundle);
      intent(c, got.stdout);
      if (c.entry === "pre-tools" && entryImplementation("toolu", c.entry) === "rust") {
        const typescript = await runCase(c, () =>
          launchedArgv({ plugin: "toolu", event: "PreToolUse", entry: "pre-tools" }, undefined, {
            ...process.env,
            TOOLU_IMPL: "",
          }),
        );
        expect(decisionOf(got.stdout).text).toBe(decisionOf(typescript.stdout).text);
        expect(got.exitCode).toBe(typescript.exitCode);
      }
      if (c.deviation === undefined) {
        expect(comparable(got)).toEqual(comparable(want));
      } else {
        expect(comparable(got).decision).not.toEqual(comparable(want).decision);
      }
    },
    CASE_TIMEOUT_MS,
  );
});

test("every golden capture belongs to a case", () => {
  expect(Object.keys(golden.cases).toSorted()).toEqual(MODULE_CASES.map((c) => c.name).toSorted());
});
