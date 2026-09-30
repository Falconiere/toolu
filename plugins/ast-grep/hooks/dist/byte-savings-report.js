#!/usr/bin/env bun
// @bun

// plugins/ast-grep/hooks/src/byte-savings-report.ts
import { existsSync, readFileSync, statSync } from "fs";
function isRecord(value) {
  if (typeof value !== "object" || value === null)
    return false;
  const r = value;
  return typeof r["kind"] === "string" && typeof r["returned"] === "number" && typeof r["full"] === "number";
}
function parseLine(line) {
  try {
    const value = JSON.parse(line);
    return isRecord(value) ? value : undefined;
  } catch {
    return;
  }
}
function report(records) {
  const byKind = new Map;
  for (const r of records) {
    const t = byKind.get(r.kind) ?? { kind: r.kind, returned: 0, full: 0, n: 0 };
    byKind.set(r.kind, {
      ...t,
      returned: t.returned + r.returned,
      full: t.full + r.full,
      n: t.n + 1
    });
  }
  const kinds = [...byKind.keys()].toSorted().flatMap((kind) => byKind.get(kind) ?? []);
  const lines = kinds.map((t) => t.kind === "read" && t.full > 0 ? `${t.kind}: returned=${t.returned} full=${t.full} saved=${Math.floor((t.full - t.returned) * 100 / t.full)}% (n=${t.n})` : `${t.kind}: returned=${t.returned} (n=${t.n})`);
  const total = kinds.reduce((sum, t) => sum + t.returned, 0);
  return [...lines, `TOTAL returned: ${total} bytes (~${Math.floor(total / 4)} tok)`].join(`
`);
}
function main(argv) {
  const ledger = argv[0] ?? "";
  if (ledger === "" || !existsSync(ledger) || !statSync(ledger).isFile()) {
    process.stderr.write(`usage: byte-savings-report.js <ledger.jsonl>
`);
    return 1;
  }
  const records = [];
  const lines = readFileSync(ledger, "utf8").split(`
`);
  for (const [index, line] of lines.entries()) {
    if (line.trim() === "")
      continue;
    const record = parseLine(line);
    if (record === undefined) {
      process.stderr.write(`byte-savings-report: ${ledger}:${index + 1}: invalid ledger line
`);
      return 1;
    }
    records.push(record);
  }
  process.stdout.write(`${report(records)}
`);
  return 0;
}
process.exitCode = main(process.argv.slice(2));
