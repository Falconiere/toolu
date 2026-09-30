/**
 * AC-1, AC-2 (#268): every search-nudge case, replayed with the TypeScript
 * module registered by `hooks/dist/register.js`, gives the stdout, stderr and
 * exit code the bash module gave at the golden's base commit, through the same
 * pre-tools bundle. A named deviation (#283 item 10) must instead show what
 * the TypeScript module does right.
 */
import { expect, test } from "bun:test";
import { NUDGE_CASES, NUDGE_DEVIATIONS } from "./cases-nudge.ts";
import { CASE_TIMEOUT_MS, expectDeviation } from "./golden-expect.ts";
import { caseKey, hostsOf, runNudge } from "./golden-harness.ts";
import { readGolden } from "./golden-io.ts";

const golden = readGolden().nudge;
const KEYS = NUDGE_CASES.flatMap((c) => hostsOf(c).map((host) => caseKey(c.name, host)));

test("the golden holds exactly the search-nudge cases", () => {
  expect(Object.keys(golden).toSorted()).toEqual(KEYS.toSorted());
});

test("no capture names a source repository", () => {
  for (const captured of Object.values(golden)) {
    expect(captured.stdout).not.toMatch(/yamless|routo|\/Volumes\/Projects\/(routo|yamless)/u);
  }
});

test("every deviation names a case", () => {
  const names = new Set(NUDGE_CASES.map((c) => c.name));
  for (const name of Object.keys(NUDGE_DEVIATIONS)) expect(names.has(name)).toBe(true);
});

for (const c of NUDGE_CASES) {
  for (const host of hostsOf(c)) {
    const key = caseKey(c.name, host);
    test.concurrent(
      key,
      async () => {
        const expected = golden[key];
        if (expected === undefined) throw new Error(`no golden capture for ${key}`);
        const actual = await runNudge(c, host, { kind: "bundle" });
        const deviation = NUDGE_DEVIATIONS[c.name];
        if (deviation === undefined) {
          expect(actual).toEqual(expected);
          return;
        }
        expectDeviation(deviation, actual.stdout, expected.stdout);
        expect({ stderr: actual.stderr, exitCode: actual.exitCode }).toEqual({
          stderr: "",
          exitCode: 0,
        });
      },
      CASE_TIMEOUT_MS,
    );
  }
}
