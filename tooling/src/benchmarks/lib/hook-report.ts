/** Summaries, the Markdown table and budget verdicts of the hook bench (#410). */
import { percentile } from "@toolu/conformance/harness/timing";
import type { Budgets, EntryResult, MeasureReport } from "./hook-data.ts";

export const MIB = 1024 * 1024;

function spread(values: number[]): { p50: number; p90: number } {
  return { p50: percentile(values, 50), p90: percentile(values, 90) };
}

/** p50/p90 of max RSS, CPU (user + sys) and wall over `samples`. */
export function summarize(
  samples: readonly MeasureReport[],
): Omit<EntryResult, "entry" | "event" | "implementation"> {
  return {
    maxRssBytes: spread(samples.map((s) => s.maxRssBytes)),
    cpuUs: spread(samples.map((s) => s.userUs + s.sysUs)),
    wallUs: spread(samples.map((s) => s.wallUs)),
  };
}

const mib = (bytes: number): string => (bytes / MIB).toFixed(1);
const ms = (us: number): string => (us / 1000).toFixed(1);

/** One Markdown row per entry: RSS in MiB, CPU and wall in ms, p50 / p90. */
export function table(rows: readonly EntryResult[]): string {
  const lines = [
    "| Entry | Event | Impl | RSS p50 / p90 (MiB) | CPU p50 / p90 (ms) | Wall p50 / p90 (ms) |",
    "|---|---|---|---|---|---|",
    ...rows.map(
      (r) =>
        `| ${r.entry} | ${r.event} | ${r.implementation} | ${mib(r.maxRssBytes.p50)} / ${mib(r.maxRssBytes.p90)} | ${ms(r.cpuUs.p50)} / ${ms(r.cpuUs.p90)} | ${ms(r.wallUs.p50)} / ${ms(r.wallUs.p90)} |`,
    ),
  ];
  return `${lines.join("\n")}\n`;
}

/**
 * One line per budget an asserted entry exceeds or lacks. `asserted` are the
 * ported entries that were measured.
 */
export function violations(
  rows: readonly EntryResult[],
  asserted: ReadonlySet<string>,
  budgets: Budgets,
  budgetsFile: string,
): string[] {
  return rows
    .filter((row) => asserted.has(row.entry))
    .flatMap((row) => {
      const tag = `${row.entry} [${row.implementation}]`;
      const budget = budgets.entries[row.entry];
      if (budget === undefined) return [`${tag}: no budget in ${budgetsFile}`];
      const found: string[] = [];
      if (row.cpuUs.p50 > budget.cpuMs * 1000) {
        found.push(`${tag}: cpu p50 ${ms(row.cpuUs.p50)} ms > budget ${String(budget.cpuMs)} ms`);
      }
      if (row.maxRssBytes.p50 > budget.maxRssMiB * MIB) {
        found.push(
          `${tag}: rss p50 ${mib(row.maxRssBytes.p50)} MiB > budget ${String(budget.maxRssMiB)} MiB`,
        );
      }
      return found;
    });
}
