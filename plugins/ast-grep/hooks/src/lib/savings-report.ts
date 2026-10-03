/**
 * A byte-savings ledger summed per tool kind (#268): bytes each tool returned
 * into context, with a ~token estimate at 4 bytes/token, and for single-file
 * Reads the share saved against reading the whole file. Shared by the report
 * CLI and by byte-savings' OpenCode report (#347).
 */
export type LedgerRecord = { kind: string; returned: number; full: number };
type KindTotal = LedgerRecord & { n: number };

function isRecord(value: unknown): value is LedgerRecord {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    typeof value.kind === "string" &&
    "returned" in value &&
    typeof value.returned === "number" &&
    "full" in value &&
    typeof value.full === "number"
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

/** Every record of `text`, blank lines skipped, or the 1-based number of the first invalid line. */
export function parseLedger(text: string): LedgerRecord[] | { line: number } {
  const records: LedgerRecord[] = [];
  for (const [index, line] of text.split("\n").entries()) {
    if (line.trim() === "") continue;
    const record = parseLine(line);
    if (record === undefined) return { line: index + 1 };
    records.push(record);
  }
  return records;
}

/** One line per kind, sorted by kind, then the total. */
export function report(records: readonly LedgerRecord[]): string {
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
