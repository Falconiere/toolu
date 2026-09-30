/**
 * Latency helpers for the epic's hook budget (#251, #247): a port's p50 may be
 * no worse than the bash baseline plus 5 ms on the same machine. Both sides are
 * measured in the same run, one process at a time, so machine speed cancels out.
 */
import type { RunResult } from "./spawn.ts";

export type Latency = { samples: number[]; p50: number; p95: number; min: number; max: number };

export class LatencyBudgetError extends Error {
  override name = "LatencyBudgetError";
}

/** Nearest-rank percentile of `values`, `p` in 0..100. */
export function percentile(values: readonly number[], p: number): number {
  if (values.length === 0) {
    throw new RangeError("percentile: no samples");
  }
  if (p < 0 || p > 100) {
    throw new RangeError(`percentile: p must be in 0..100, got ${p}`);
  }
  const sorted = values.toSorted((a, b) => a - b);
  const rank = Math.max(1, Math.ceil((p / 100) * sorted.length));
  return sorted[rank - 1] ?? Number.NaN;
}

function sampleOf(reading: RunResult | number): number {
  if (typeof reading === "number") {
    return reading;
  }
  if (reading.timedOut) {
    throw new LatencyBudgetError(
      `measured run timed out after ${reading.durationMs.toFixed(0)} ms`,
    );
  }
  return reading.durationMs;
}

/** Take `count` readings strictly one after another: concurrent spawns would contend and skew each other. */
function sequential(once: () => Promise<RunResult | number>, count: number): Promise<number[]> {
  const samples: number[] = [];
  return Array.from({ length: count })
    .reduce<Promise<void>>(async (previous) => {
      await previous;
      samples.push(sampleOf(await once()));
    }, Promise.resolve())
    .then(() => samples);
}

/**
 * Run `once` `warmup` times (discarded) then `runs` times, sequentially. A
 * reading is a RunResult (its durationMs) or a millisecond number.
 */
export async function measureLatency(
  once: () => Promise<RunResult | number>,
  opts: { runs: number; warmup?: number },
): Promise<Latency> {
  if (opts.runs < 1) {
    throw new RangeError(`measureLatency: runs must be >= 1, got ${opts.runs}`);
  }
  await sequential(once, opts.warmup ?? 0);
  return latencyOf(await sequential(once, opts.runs));
}

/** Alternate two prepared sides, one spawn at a time, so changing load affects both. */
export async function measureAlternatingLatency<T>(
  sides: readonly [T, T],
  once: (side: T) => Promise<RunResult | number>,
  opts: { runs: number; warmup?: number },
): Promise<readonly [Latency, Latency]> {
  if (opts.runs < 1) {
    throw new RangeError(`measureAlternatingLatency: runs must be >= 1, got ${opts.runs}`);
  }
  const samples: [number[], number[]] = [[], []];
  const warmup = opts.warmup ?? 0;
  const calls = [...Array(warmup + opts.runs).keys()].flatMap((round) =>
    ([0, 1] as const).map((index) => ({ round, index })),
  );
  await calls.reduce<Promise<void>>(async (previous, { round, index }) => {
    await previous;
    const reading = sampleOf(await once(sides[index]));
    if (round >= warmup) samples[index].push(reading);
  }, Promise.resolve());
  return [latencyOf(samples[0]), latencyOf(samples[1])];
}

/** The summary of `samples` (milliseconds), taken however the caller interleaved them. */
export function latencyOf(samples: readonly number[]): Latency {
  return {
    samples: [...samples],
    p50: percentile(samples, 50),
    p95: percentile(samples, 95),
    min: Math.min(...samples),
    max: Math.max(...samples),
  };
}

const formatMs = (value: number): string => value.toFixed(1);

/** Shared Markdown comparison used by the language and structural hook benchmarks. */
export function latencyComparisonTable(
  rows: readonly { name: string; bash: Latency; ts: Latency }[],
): string {
  const body = rows.map(
    (row) =>
      `| ${row.name} | ${formatMs(row.bash.p50)} | ${formatMs(row.ts.p50)} | ${formatMs(row.ts.p50 - row.bash.p50)} |`,
  );
  const head = ["| Fixture | bash module p50 | TS module p50 | TS − bash |", "|---|---|---|---|"];
  return `${[...head, ...body].join("\n")}\n`;
}

/** Throw unless `candidate` p50 is within `budgetMs` of `baseline` p50. */
export function assertLatencyBudget(candidate: Latency, baseline: Latency, budgetMs: number): void {
  if (candidate.p50 > baseline.p50 + budgetMs) {
    throw new LatencyBudgetError(
      `p50 ${candidate.p50.toFixed(2)} ms exceeds baseline ${baseline.p50.toFixed(2)} ms + ${budgetMs} ms`,
    );
  }
}
