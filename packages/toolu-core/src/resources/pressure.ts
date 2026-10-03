/** Bounded machine sampling and replayable sustained-pressure decisions. */
import { readFileSync } from "node:fs";
import { cpus, freemem, loadavg, totalmem } from "node:os";
import { isJsonObject } from "../config/config-load.ts";
import { errno } from "./lock.ts";

export type ResourceSample = {
  at: number;
  cpus: number;
  load: number;
  availableBytes: number;
  totalBytes: number;
  steal: number | null;
  ticks?: { total: number; steal: number };
};
export type Pressure = {
  held: boolean;
  badSince: number | null;
  goodSince: number | null;
  sample: ResourceSample;
  reason: string | null;
};

export const PRESSURE_SAMPLE_MS = 30_000;
export const PRESSURE_HOLD_MS = 60_000;
export const PRESSURE_RECOVERY_MS = 120_000;

function finite(value: number): boolean {
  return Number.isFinite(value);
}

function isSample(sample: unknown): sample is ResourceSample {
  if (!isJsonObject(sample)) return false;
  const ticks = sample.ticks;
  return (
    typeof sample.at === "number" &&
    finite(sample.at) &&
    sample.at >= 0 &&
    typeof sample.cpus === "number" &&
    Number.isSafeInteger(sample.cpus) &&
    sample.cpus > 0 &&
    typeof sample.load === "number" &&
    finite(sample.load) &&
    sample.load >= 0 &&
    typeof sample.availableBytes === "number" &&
    finite(sample.availableBytes) &&
    sample.availableBytes >= 0 &&
    typeof sample.totalBytes === "number" &&
    finite(sample.totalBytes) &&
    sample.totalBytes > 0 &&
    sample.availableBytes <= sample.totalBytes &&
    (sample.steal === null ||
      (typeof sample.steal === "number" &&
        finite(sample.steal) &&
        sample.steal >= 0 &&
        sample.steal <= 1)) &&
    (ticks === undefined ||
      (isJsonObject(ticks) &&
        typeof ticks.total === "number" &&
        Number.isSafeInteger(ticks.total) &&
        ticks.total >= 0 &&
        typeof ticks.steal === "number" &&
        Number.isSafeInteger(ticks.steal) &&
        ticks.steal >= 0))
  );
}

export function isPressure(value: unknown): value is Pressure {
  if (!isJsonObject(value)) return false;
  const badSinceValid =
    value.badSince === null ||
    (typeof value.badSince === "number" &&
      finite(value.badSince) &&
      isSample(value.sample) &&
      value.badSince <= value.sample.at);
  const goodSinceValid =
    value.goodSince === null ||
    (typeof value.goodSince === "number" &&
      finite(value.goodSince) &&
      isSample(value.sample) &&
      value.goodSince <= value.sample.at);
  return (
    typeof value.held === "boolean" &&
    badSinceValid &&
    goodSinceValid &&
    isSample(value.sample) &&
    (value.held ? typeof value.reason === "string" : value.reason === null)
  );
}

export function advancePressure(previous: Pressure | undefined, sample: ResourceSample): Pressure {
  if (!isSample(sample) || (previous !== undefined && !isPressure(previous))) {
    throw new Error("invalid resource pressure sample");
  }
  if (previous !== undefined && sample.at < previous.sample.at) {
    throw new Error("resource pressure sample moved backwards");
  }
  const memory = sample.availableBytes / sample.totalBytes;
  const effective = Math.max(0.1, sample.cpus * (1 - (sample.steal ?? 0)));
  const bad = sample.load > effective * 1.5 || memory < 0.1 || (sample.steal ?? 0) > 0.25;
  const good = sample.load < effective * 0.8 && memory > 0.2 && (sample.steal ?? 0) < 0.1;
  const badSince = bad ? (previous?.badSince ?? sample.at) : null;
  const goodSince = good ? (previous?.goodSince ?? sample.at) : null;
  let held = previous?.held ?? false;
  if (badSince !== null && sample.at - badSince >= PRESSURE_HOLD_MS) held = true;
  if (goodSince !== null && sample.at - goodSince >= PRESSURE_RECOVERY_MS) held = false;
  return {
    held,
    badSince,
    goodSince,
    sample,
    reason: held ? "sustained CPU/load or memory pressure" : null,
  };
}

export function sampleResources(previous?: ResourceSample): ResourceSample {
  const sample: ResourceSample = {
    at: Date.now(),
    cpus: cpus().length,
    load: loadavg()[0] ?? 0,
    availableBytes: freemem(),
    totalBytes: totalmem(),
    steal: null,
  };
  if (process.platform !== "linux") return sample;
  try {
    const cpu =
      readFileSync("/proc/stat", "utf8").split("\n")[0]?.trim().split(/\s+/).slice(1).map(Number) ??
      [];
    // guest fields are already included in user/nice; exclude them from total.
    const ticks = { total: cpu.slice(0, 8).reduce((sum, n) => sum + n, 0), steal: cpu[7] ?? 0 };
    const before = previous?.ticks;
    if (before && ticks.total > before.total)
      sample.steal = Math.max(0, (ticks.steal - before.steal) / (ticks.total - before.total));
    sample.ticks = ticks;
    const available = /^MemAvailable:\s+(\d+) kB$/m.exec(readFileSync("/proc/meminfo", "utf8"));
    if (available?.[1]) sample.availableBytes = Number(available[1]) * 1024;
  } catch (error) {
    if (!errno(error, "ENOENT")) throw error;
  }
  return sample;
}
