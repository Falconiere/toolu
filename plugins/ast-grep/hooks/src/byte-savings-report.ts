#!/usr/bin/env bun
/**
 * byte-savings-report <ledger.jsonl> (#268): sums a per-session byte-savings
 * ledger per tool kind: bytes each tool returned into context (with a ~token
 * estimate at 4 bytes/token) and, for single-file Reads, the share saved
 * against reading the whole file. Read-only. Port of `byte-savings-report.sh`.
 */
import { existsSync, readFileSync, statSync } from "node:fs";

type LedgerRecord = { kind: string; returned: number; full: number };
type KindTotal = LedgerRecord & { n: number };

function isRecord(value: unknown): value is LedgerRecord {
  if (typeof value !== "object" || value === null) return false;
  const r = value as Record<string, unknown>;
  return (
    typeof r["kind"] === "string" &&
    typeof r["returned"] === "number" &&
    typeof r["full"] === "number"
  );
}

function parseLine(line: string): LedgerRecord | undefined {
  try {
    const value: unknown = JSON.parse(line);
    return isRecord(value) ? value : undefined;
  } catch {
    return undefined;
  }
}

/** One line per kind, sorted by kind, then the total. */
function report(records: readonly LedgerRecord[]): string {
  const byKind = new Map<string, KindTotal>();
  for (const r of records) {
    const t = byKind.get(r.kind) ?? { kind: r.kind, returned: 0, full: 0, n: 0 };
    byKind.set(r.kind, {
      ...t,
      returned: t.returned + r.returned,
      full: t.full + r.full,
      n: t.n + 1,
    });
  }
  const kinds = [...byKind.keys()].toSorted().flatMap((kind) => byKind.get(kind) ?? []);
  const lines = kinds.map((t) =>
    t.kind === "read" && t.full > 0
      ? `${t.kind}: returned=${t.returned} full=${t.full} saved=${Math.floor(((t.full - t.returned) * 100) / t.full)}% (n=${t.n})`
      : `${t.kind}: returned=${t.returned} (n=${t.n})`,
  );
  const total = kinds.reduce((sum, t) => sum + t.returned, 0);
  return [...lines, `TOTAL returned: ${total} bytes (~${Math.floor(total / 4)} tok)`].join("\n");
}

function main(argv: readonly string[]): number {
  const ledger = argv[0] ?? "";
  if (ledger === "" || !existsSync(ledger) || !statSync(ledger).isFile()) {
    process.stderr.write("usage: byte-savings-report.js <ledger.jsonl>\n");
    return 1;
  }
  const records: LedgerRecord[] = [];
  const lines = readFileSync(ledger, "utf8").split("\n");
  for (const [index, line] of lines.entries()) {
    if (line.trim() === "") continue;
    const record = parseLine(line);
    if (record === undefined) {
      process.stderr.write(`byte-savings-report: ${ledger}:${index + 1}: invalid ledger line\n`);
      return 1;
    }
    records.push(record);
  }
  process.stdout.write(`${report(records)}\n`);
  return 0;
}

process.exitCode = main(process.argv.slice(2));
