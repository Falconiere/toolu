/**
 * AC-1, AC-2, AC-10 (#265): every golden case, replayed with the TypeScript
 * module registered by `hooks/dist/register.js`, gives the stdout, stderr,
 * exit code and project state (gate file, telemetry) the bash module gave at
 * the golden's base commit, through the same post-tools bundle. A named
 * deviation must instead show what the TypeScript module does right.
 */
import { expect, test } from "bun:test";
import { isJsonObject } from "@toolu/core/config";
import { TS_CASES } from "./cases.ts";
import type { TsCase } from "./cases-types.ts";
import { caseKey, runCase, type StepResult } from "./golden-harness.ts";
import { readGolden } from "./golden-io.ts";

const golden = readGolden();

/** Each case spawns real git, bash and Bun processes, several times. */
const CASE_TIMEOUT_MS = 60_000;

/** Every string in `value` as its sorted lines: order dropped, content kept. */
function linesSorted(value: unknown): unknown {
  if (typeof value === "string") return value.split("\n").toSorted();
  if (Array.isArray(value)) return value.map(linesSorted);
  if (isJsonObject(value)) {
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, linesSorted(v)]));
  }
  return value;
}

/** JSON text compared by content with line order dropped; other text by its sorted lines. */
function unordered(text: string): unknown {
  try {
    return linesSorted(JSON.parse(text));
  } catch {
    return linesSorted(text);
  }
}

/** A step as compared: exact, or with line order dropped for output whose order bash never fixed. */
function comparable(c: TsCase, step: StepResult) {
  if (c.unordered !== true) return step;
  const state = Object.fromEntries(Object.entries(step.state).map(([k, v]) => [k, unordered(v)]));
  return { ...step, stdout: unordered(step.stdout), state };
}

for (const c of TS_CASES) {
  for (const host of c.hosts ?? ["claude"]) {
    const key = caseKey(c, host);
    test.concurrent(
      key,
      async () => {
        const expected = golden.cases[key];
        if (expected === undefined) throw new Error(`no golden capture for ${key}`);
        const actual = await runCase(c, host, { kind: "bundle" });
        const deviation = c.deviation;
        if (deviation !== undefined) {
          const last = actual.at(-1)?.stdout ?? "";
          for (const text of deviation.contains) expect(last).toContain(text);
          expect(last).not.toBe(expected.at(-1)?.stdout);
          return;
        }
        expect(actual.map((step) => comparable(c, step))).toEqual(
          expected.map((step) => comparable(c, step)),
        );
      },
      CASE_TIMEOUT_MS,
    );
  }
}
