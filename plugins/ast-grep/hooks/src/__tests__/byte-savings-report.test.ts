/**
 * AC-6 (#268): the report bundle prints what the bash report printed at the
 * golden's base commit for each report case, including the three
 * byte-savings-report.bats ledgers. Where bash printed a jq error (an empty
 * ledger, a line that is not a ledger record) the Bun CLI's answer is pinned.
 */
import { expect, test } from "bun:test";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { REPORT_CASES, REPORT_DEVIATIONS } from "./cases-report.ts";
import { CASE_TIMEOUT_MS } from "./golden-expect.ts";
import { runReport } from "./golden-harness.ts";
import { readGolden } from "./golden-io.ts";
import { PLUGIN_ROOT } from "./golden-sandbox.ts";

const golden = readGolden().report;
const REPORT = join(PLUGIN_ROOT, "hooks/dist/byte-savings-report.js");

test("the golden holds exactly the report cases", () => {
  expect(Object.keys(golden).toSorted()).toEqual(REPORT_CASES.map((c) => c.name).toSorted());
});

for (const c of REPORT_CASES) {
  test.concurrent(
    c.name,
    async () => {
      const expected = golden[c.name];
      if (expected === undefined) throw new Error(`no golden capture for ${c.name}`);
      const actual = await runReport(c, { kind: "bundle" });
      const stderr = REPORT_DEVIATIONS[c.name];
      expect(actual).toEqual(stderr === undefined ? expected : { ...expected, stderr });
    },
    CASE_TIMEOUT_MS,
  );
}

async function reportOn(ledger: string) {
  using sb = createSandbox();
  const file = sb.path("ledger.jsonl");
  writeFileSync(file, ledger);
  const res = await run([process.execPath, REPORT, file], { cwd: sb.root });
  return { ...res, stderr: res.stderr.split(file).join("<LEDGER>") };
}

test.concurrent("an empty ledger reports a zero total", async () => {
  expect(await reportOn("")).toMatchObject({
    exitCode: 0,
    stdout: "TOTAL returned: 0 bytes (~0 tok)\n",
    stderr: "",
  });
});

test.concurrent("blank lines are skipped", async () => {
  const res = await reportOn('\n{"kind":"grep","returned":8,"full":0}\n\n');
  expect(res).toMatchObject({
    exitCode: 0,
    stdout: "grep: returned=8 (n=1)\nTOTAL returned: 8 bytes (~2 tok)\n",
  });
});

for (const line of ["not json", '{"kind":"grep","returned":"8","full":0}', "[1,2]"]) {
  test.concurrent(`a line that is not a ledger record fails with its line number: ${line}`, async () => {
    const res = await reportOn(`{"kind":"grep","returned":8,"full":0}\n${line}\n`);
    expect(res).toMatchObject({
      exitCode: 1,
      stdout: "",
      stderr: "byte-savings-report: <LEDGER>:2: invalid ledger line\n",
    });
  });
}
