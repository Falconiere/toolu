/**
 * byte-savings-report golden cases (#268): the three byte-savings-report.bats
 * ledgers, then ordering across kinds and a Read that returned more than the
 * whole file. An empty ledger and a non-JSON line are not captured: bash
 * printed a jq error there, and `byte-savings-report.test.ts` pins what the
 * Bun CLI prints instead.
 */
import type { ReportCase } from "./cases-types.ts";

const lines = (...records: object[]) => records.map((r) => `${JSON.stringify(r)}\n`).join("");

export const REPORT_CASES: readonly ReportCase[] = [
  {
    name: "bats: per-kind summary with Read saving% and totals",
    ledger: lines(
      { kind: "read", returned: 120, full: 4000 },
      { kind: "grep", returned: 300, full: 0 },
      { kind: "ast-grep", returned: 80, full: 0 },
    ),
  },
  {
    name: "bats: sums repeated kinds across a session",
    ledger: lines(
      { kind: "read", returned: 100, full: 1000 },
      { kind: "read", returned: 100, full: 1000 },
    ),
  },
  { name: "bats: missing ledger argument fails with usage", noArgument: true },
  { name: "a ledger path that does not exist" },
  {
    name: "kinds sort by name; a Read with no full size has no saving",
    ledger: lines(
      { kind: "read", returned: 7, full: 0 },
      { kind: "glob", returned: 9, full: 0 },
      { kind: "ast-grep", returned: 3, full: 0 },
      { kind: "grep", returned: 2, full: 0 },
    ),
  },
  {
    name: "a Read that returned more than the file saves a negative share",
    ledger: lines(
      { kind: "read", returned: 150, full: 100 },
      { kind: "read", returned: 1, full: 7 },
    ),
  },
];

/** The usage line names the Bun CLI that replaced the bash script: the stderr it prints instead. */
export const REPORT_DEVIATIONS: Readonly<Record<string, string>> = {
  "bats: missing ledger argument fails with usage":
    "usage: byte-savings-report.js <ledger.jsonl>\n",
  "a ledger path that does not exist": "usage: byte-savings-report.js <ledger.jsonl>\n",
};
