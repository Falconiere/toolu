/**
 * AC-3 (#268): every byte-savings case, replayed with the TypeScript module
 * registered by `hooks/dist/register.js`, leaves the ledgers, stdout, stderr
 * and exit code the bash module left at the golden's base commit, through the
 * same post-tools bundle. A named deviation (#283 item 10) must instead show
 * the ledger the TypeScript module writes.
 */
import { expect, test } from "bun:test";
import { SAVINGS_CASES } from "./cases-savings.ts";
import { CASE_TIMEOUT_MS, expectDeviation } from "./golden-expect.ts";
import { caseKey, hostsOf, runSavings } from "./golden-harness.ts";
import { readGolden } from "./golden-io.ts";

const golden = readGolden().savings;
const KEYS = SAVINGS_CASES.flatMap((c) => hostsOf(c).map((host) => caseKey(c.name, host)));

function ledgerText(ledgers: Record<string, string>): string {
  return Object.values(ledgers).join("");
}

test("the golden holds exactly the byte-savings cases", () => {
  expect(Object.keys(golden).toSorted()).toEqual(KEYS.toSorted());
});

test("the baseline has two named deviations", () => {
  expect(SAVINGS_CASES.filter((c) => c.deviation !== undefined).length).toBe(2);
});

for (const c of SAVINGS_CASES) {
  for (const host of hostsOf(c)) {
    const key = caseKey(c.name, host);
    test.concurrent(
      key,
      async () => {
        const expected = golden[key];
        if (expected === undefined) throw new Error(`no golden capture for ${key}`);
        const actual = await runSavings(c, host, { kind: "bundle" });
        const deviation = c.deviation;
        if (deviation === undefined) {
          expect(actual).toEqual(expected);
          return;
        }
        expectDeviation(deviation, ledgerText(actual.ledgers), ledgerText(expected.ledgers));
        expect({ stdout: actual.stdout, stderr: actual.stderr, exitCode: actual.exitCode }).toEqual(
          {
            stdout: expected.stdout,
            stderr: expected.stderr,
            exitCode: expected.exitCode,
          },
        );
      },
      CASE_TIMEOUT_MS,
    );
  }
}
