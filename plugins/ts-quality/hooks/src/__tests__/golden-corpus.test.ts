/**
 * The bash golden (#265) is complete and says what the bats suites said:
 * every case and host was captured from a real base commit, each step exited
 * 0, and the last step's decision class and text match the case's
 * expectations — the assertions the deleted bats files made.
 */
import { expect, test } from "bun:test";
import { outcomeOf } from "./golden-outcome.ts";
import { TS_CASES } from "./cases.ts";
import { caseKey } from "./golden-harness.ts";
import { readGolden } from "./golden-io.ts";

const golden = readGolden();

test("the golden holds exactly the corpus's cases", () => {
  const keys = TS_CASES.flatMap((c) => (c.hosts ?? ["claude"]).map((host) => caseKey(c, host)));
  expect(new Set(keys).size).toBe(keys.length);
  expect(Object.keys(golden.cases).toSorted()).toEqual(keys.toSorted());
});

for (const c of TS_CASES) {
  for (const host of c.hosts ?? ["claude"]) {
    test(`bash capture meets the case: ${caseKey(c, host)}`, () => {
      const steps = golden.cases[caseKey(c, host)] ?? [];
      expect(steps.length).toBe(c.steps.length);
      for (const step of steps) expect(step.exitCode).toBe(0);
      const last = steps.at(-1);
      if (last === undefined) throw new Error("no steps");
      expect(outcomeOf(last.stdout, last.exitCode)).toBe(c.expect);
      for (const text of c.contains ?? []) expect(last.stdout).toContain(text);
      for (const text of c.absent ?? []) expect(last.stdout).not.toContain(text);
    });
  }
}
