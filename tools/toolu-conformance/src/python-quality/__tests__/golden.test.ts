/**
 * #266, #427: every golden case, replayed with the native rule registered by
 * `toolu python-quality hook register`, gives the stdout, stderr, exit code and
 * project state (gate file, telemetry) the bash module gave at the golden's
 * base commit, both through the native `toolu hook post-tools` and through the
 * Bun post-tools bundle with its bridge module. A named deviation must instead
 * show what the port does right.
 */
import { expect, test } from "bun:test";
import { OWN_WORKTREE_GATE, PY_CASES } from "../cases.ts";
import type { PyCase } from "../cases-types.ts";
import { caseKey, runCase, type Leg, type StepResult } from "../golden-harness.ts";
import { readGolden } from "../golden-io.ts";

const golden = readGolden();

/** Each case spawns real git, bash and Bun processes, several times. */
const CASE_TIMEOUT_MS = 60_000;

const LEGS: readonly Leg[] = ["native", "bun"];

function defineCase(c: PyCase, host: "claude" | "codex", leg: Leg): void {
  const key = caseKey(c, host);
  test.concurrent(
    `${key} [${leg}]`,
    async () => {
      const expected = golden.cases[key];
      if (expected === undefined) throw new Error(`no golden capture for ${key}`);
      const actual = await runCase(c, host, { kind: "native" }, leg);
      const deviation = c.deviation;
      if (c.name === OWN_WORKTREE_GATE) {
        const output = ({ state: _, ...rest }: StepResult) => rest;
        expect(actual.map(output)).toEqual(expected.map(output));
        expect(actual.map((step) => step.state)).toEqual(expected.map(() => ({})));
        return;
      }
      if (deviation !== undefined) {
        const last = actual.at(-1)?.stdout ?? "";
        for (const text of deviation.contains) expect(last).toContain(text);
        expect(last).not.toBe(expected.at(-1)?.stdout);
        return;
      }
      expect(actual).toEqual(expected);
    },
    CASE_TIMEOUT_MS,
  );
}

for (const leg of LEGS) {
  for (const c of PY_CASES) {
    for (const host of c.hosts ?? ["claude"]) {
      defineCase(c, host, leg);
    }
  }
}
