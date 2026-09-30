/**
 * The bash golden (#267) is complete and says what the bats suites said:
 * every case and host was captured from a real base commit, each step exited
 * 0, the last step's decision class and text match the case's expectations —
 * the assertions the deleted bats files made — and every bats test at the
 * base is ported by a case or named in `NOT_GOLDEN`.
 */
import { expect, test } from "bun:test";
import { outcomeOf } from "./golden-outcome.ts";
import { NOT_GOLDEN, RS_CASES } from "./cases.ts";
import { BASH_BASE, REPO_ROOT, caseKey } from "./golden-harness.ts";
import { readGolden } from "./golden-io.ts";

const golden = readGolden();

const SUITES = [
  "assembled",
  "dispatch",
  "docs",
  "error-handling",
  "no-mocks",
  "size",
  "suppression",
  "tests",
  "unsafe",
];

function batsTitles(suite: string): string[] {
  const shown = Bun.spawnSync(
    ["git", "show", `${BASH_BASE}:plugins/rust-quality/hooks/concerns/__tests__/${suite}.bats`],
    { cwd: REPO_ROOT },
  );
  if (shown.exitCode !== 0)
    throw new Error(`git show ${suite}.bats failed: ${shown.stderr.toString()}`);
  return [...shown.stdout.toString().matchAll(/^@test "((?:[^"\\]|\\.)*)" \{$/gm)].map(
    (m) => `${suite}.bats: ${(m[1] ?? "").replaceAll('\\"', '"')}`,
  );
}

test("the golden holds exactly the corpus's cases", () => {
  const keys = RS_CASES.flatMap((c) => (c.hosts ?? ["claude"]).map((host) => caseKey(c, host)));
  expect(new Set(keys).size).toBe(keys.length);
  expect(Object.keys(golden.cases).toSorted()).toEqual(keys.toSorted());
});

test("every bats test at the base is ported by a case or named as not golden", () => {
  const titles = SUITES.flatMap(batsTitles);
  expect(titles.length).toBeGreaterThan(80);
  const ported = new Set([...RS_CASES.flatMap((c) => c.from ?? []), ...NOT_GOLDEN]);
  expect(titles.filter((t) => !ported.has(t))).toEqual([]);
  expect([...ported].filter((t) => !titles.includes(t))).toEqual([]);
});

for (const c of RS_CASES) {
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
