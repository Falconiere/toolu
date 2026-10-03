#!/usr/bin/env bun
/**
 * byte-savings-report <ledger.jsonl> (#268): sums a per-session byte-savings
 * ledger per tool kind: bytes each tool returned into context (with a ~token
 * estimate at 4 bytes/token) and, for single-file Reads, the share saved
 * against reading the whole file. Read-only. Port of `byte-savings-report.sh`.
 */
import { existsSync, readFileSync, statSync } from "node:fs";
import { parseLedger, report } from "./lib/savings-report.ts";

function main(argv: readonly string[]): number {
  const ledger = argv[0] ?? "";
  if (ledger === "" || !existsSync(ledger) || !statSync(ledger).isFile()) {
    process.stderr.write("usage: byte-savings-report.js <ledger.jsonl>\n");
    return 1;
  }
  const records = parseLedger(readFileSync(ledger, "utf8"));
  if (!Array.isArray(records)) {
    process.stderr.write(`byte-savings-report: ${ledger}:${records.line}: invalid ledger line\n`);
    return 1;
  }
  process.stdout.write(`${report(records)}\n`);
  return 0;
}

process.exitCode = main(process.argv.slice(2));
