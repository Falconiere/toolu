/**
 * AC-1 (#267): every golden case, replayed with the TypeScript module
 * registered by `hooks/dist/register.js`, gives the stdout, stderr, exit code
 * and project state (gate file, telemetry) the bash module gave at the
 * golden's base commit, through the same post-tools bundle. A named deviation
 * must instead show what the TypeScript module does right.
 */
import { expect, test } from "bun:test";
import { DEVIATIONS, RS_CASES } from "./cases.ts";
import { caseKey, runCase } from "./golden-harness.ts";
import { readGolden } from "./golden-io.ts";

const golden = readGolden();

/** Each case spawns real git, bash and Bun processes, several times. */
const CASE_TIMEOUT_MS = 60_000;

for (const c of RS_CASES) {
  for (const host of c.hosts ?? ["claude"]) {
    const key = caseKey(c, host);
    test.concurrent(
      key,
      async () => {
        const expected = golden.cases[key];
        if (expected === undefined) throw new Error(`no golden capture for ${key}`);
        const actual = await runCase(c, host, { kind: "bundle" });
        const deviation = DEVIATIONS[c.name];
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
}
