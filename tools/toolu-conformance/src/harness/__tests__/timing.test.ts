import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { run } from "../spawn.ts";
import {
  assertLatencyBudget,
  latencyComparisonTable,
  LatencyBudgetError,
  measureAlternatingLatency,
  measureLatency,
  percentile,
  type Latency,
} from "../timing.ts";

const SAMPLE = join(
  resolve(import.meta.dir, "../../../../.."),
  "plugins/toolu/hooks/dist/sample.js",
);

/** The rejection of `promise`, or null when it resolved. */
function rejection(promise: Promise<unknown>): Promise<unknown> {
  return promise.then(
    () => null,
    (err: unknown) => err,
  );
}

function latency(p50: number): Latency {
  return { samples: [p50], p50, p95: p50, min: p50, max: p50 };
}

test.concurrent("percentile uses nearest rank and rejects empty input or an out-of-range p", () => {
  const values = [5, 1, 4, 2, 3];
  expect(percentile(values, 50)).toBe(3);
  expect(percentile(values, 95)).toBe(5);
  expect(percentile(values, 0)).toBe(1);
  expect(percentile(values, 100)).toBe(5);
  expect(() => percentile([], 50)).toThrow("no samples");
  expect(() => percentile(values, 101)).toThrow("0..100");
  expect(() => percentile(values, -1)).toThrow("0..100");
});

test.concurrent("the budget passes at and under baseline + budget and fails above it", () => {
  expect(() => assertLatencyBudget(latency(10), latency(6), 5)).not.toThrow();
  expect(() => assertLatencyBudget(latency(11), latency(6), 5)).not.toThrow();
  expect(() => assertLatencyBudget(latency(11.5), latency(6), 5)).toThrow(LatencyBudgetError);
  expect(() => assertLatencyBudget(latency(11.5), latency(6), 5)).toThrow(
    "p50 11.50 ms exceeds baseline 6.00 ms + 5 ms",
  );
});

// Timing measures one process at a time, so this test stays serial by design.
test("measureLatency samples real bun hook spawns sequentially", async () => {
  const lat = await measureLatency(async () => run([process.execPath, SAMPLE]), {
    runs: 5,
    warmup: 1,
  });
  expect(lat.samples).toHaveLength(5);
  expect(lat.min).toBeGreaterThan(0);
  expect(lat.min).toBeLessThanOrEqual(lat.p50);
  expect(lat.p50).toBeLessThanOrEqual(lat.p95);
  expect(lat.p95).toBeLessThanOrEqual(lat.max);
});

test.concurrent("measureLatency accepts plain millisecond readings and rejects runs < 1", async () => {
  let n = 0;
  const lat = await measureLatency(() => Promise.resolve(++n), { runs: 3 });
  expect(lat.samples).toEqual([1, 2, 3]);
  expect(lat.p50).toBe(2);
  const err = await rejection(measureLatency(() => Promise.resolve(1), { runs: 0 }));
  expect(err).toBeInstanceOf(RangeError);
  expect(String(err)).toContain("runs must be >= 1");
});

test.concurrent("alternating latency samples each side in order and discards warmup", async () => {
  const order: string[] = [];
  let reading = 0;
  const [bash, ts] = await measureAlternatingLatency(
    ["bash", "ts"],
    (side) => {
      order.push(side);
      return Promise.resolve(++reading);
    },
    { runs: 2, warmup: 1 },
  );
  expect(order).toEqual(["bash", "ts", "bash", "ts", "bash", "ts"]);
  expect(bash.samples).toEqual([3, 5]);
  expect(ts.samples).toEqual([4, 6]);
  expect(bash.p50).toBe(3);
  expect(ts.p50).toBe(4);
  const err = await rejection(
    measureAlternatingLatency([0, 1], () => Promise.resolve(1), { runs: 0 }),
  );
  expect(err).toBeInstanceOf(RangeError);
});

test.concurrent("latency comparison table includes both medians and their delta", () => {
  expect(latencyComparisonTable([{ name: "clean", bash: latency(300), ts: latency(145) }])).toBe(
    "| Fixture | bash module p50 | TS module p50 | TS − bash |\n" +
      "|---|---|---|---|\n" +
      "| clean | 300.0 | 145.0 | -155.0 |\n",
  );
});

test.concurrent("a timed-out run is reported rather than timed; a deny (exit 2) is still a latency", async () => {
  const err = await rejection(
    measureLatency(() => run(["sleep", "5"], { timeoutMs: 50 }), { runs: 1 }),
  );
  expect(err).toBeInstanceOf(LatencyBudgetError);
  expect(String(err)).toContain("timed out");
  const deny = await measureLatency(() => run(["sh", "-c", "exit 2"]), { runs: 1 });
  expect(deny.samples).toHaveLength(1);
});
