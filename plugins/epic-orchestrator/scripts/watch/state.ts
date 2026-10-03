/** Persisted watcher scheduling, alert acknowledgement, and CLI validation. */

import { existsSync } from "node:fs";
import { envNumber } from "../ratelimit.ts";
import { readJson } from "../common.ts";

const DEFAULT_STALL_REPEAT = 30;

export type SeenMark = {
  reported?: string;
  gone?: boolean;
  blocked?: boolean;
  stalled?: string;
  stall?: { stamp: string; nextAlertAt: number; alerts: number; acknowledgedAt?: string };
  limitScan?: string | undefined;
  limited?: string;
};

export type WatchRuntime = {
  version: 1;
  nextCheckpointAt: number;
  checkpointQueue: string[];
  nextBudgetAt: number;
  budgetAlertReset: number;
  budgetHoldUntil: number;
  herdrFailures: number;
  herdrRetryAt: number;
};

export type WatchOpts = {
  stateDir: string;
  interval: number;
  maxWait: number;
  recheck: number;
  checkpointMin: number;
  peek: boolean;
  ack?: string;
};

export function initialWatchRuntime(now: number): WatchRuntime {
  return {
    version: 1,
    nextCheckpointAt: now,
    checkpointQueue: [],
    nextBudgetAt: now,
    budgetAlertReset: 0,
    budgetHoldUntil: 0,
    herdrFailures: 0,
    herdrRetryAt: 0,
  };
}

function validTime(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0;
}

export function readWatchRuntime(path: string, now: number): WatchRuntime {
  if (!existsSync(path)) return initialWatchRuntime(now);
  const value = readJson<unknown>(path, null);
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`invalid watcher state: ${path}`);
  }
  const state = value as Partial<WatchRuntime>;
  if (
    state.version !== 1 ||
    !validTime(state.nextCheckpointAt) ||
    !Array.isArray(state.checkpointQueue) ||
    state.checkpointQueue.some((key) => typeof key !== "string") ||
    !validTime(state.nextBudgetAt) ||
    !validTime(state.budgetAlertReset) ||
    !validTime(state.budgetHoldUntil) ||
    !Number.isSafeInteger(state.herdrFailures) ||
    (state.herdrFailures ?? -1) < 0 ||
    !validTime(state.herdrRetryAt)
  ) {
    throw new Error(`invalid watcher state: ${path}`);
  }
  return state as WatchRuntime;
}

function requireTiming(value: number, label: string, allowZero: boolean): void {
  if (!Number.isFinite(value) || (allowZero ? value < 0 : value <= 0)) {
    throw new Error(
      `${label} must be ${allowZero ? "a non-negative" : "a positive"} finite number`,
    );
  }
}

export function parseWatchArgs(argv: string[]): WatchOpts {
  const options: WatchOpts = {
    stateDir: "",
    interval: 60,
    maxWait: 2700,
    recheck: 300,
    checkpointMin: 15,
    peek: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--state-dir") options.stateDir = argv[++index] ?? "";
    else if (arg === "--interval") options.interval = Number(argv[++index]);
    else if (arg === "--max-wait") options.maxWait = Number(argv[++index]);
    else if (arg === "--recheck") options.recheck = Number(argv[++index]);
    else if (arg === "--checkpoint") options.checkpointMin = Number(argv[++index]);
    else if (arg === "--peek") options.peek = true;
    else if (arg === "--ack") options.ack = argv[++index] ?? "";
    else throw new Error(`unknown arg: ${arg}`);
  }
  if (!options.stateDir) throw new Error("--state-dir is required");
  if (options.ack === "") throw new Error("--ack needs an issue key");
  if (options.peek && options.ack !== undefined) {
    throw new Error("--peek and --ack cannot be combined");
  }
  requireTiming(options.interval, "--interval", false);
  requireTiming(options.maxWait, "--max-wait", true);
  requireTiming(options.recheck, "--recheck", true);
  requireTiming(options.checkpointMin, "--checkpoint", true);
  if (options.peek) options.maxWait = 0;
  return options;
}

export function acknowledgeStall(
  seen: Record<string, SeenMark>,
  key: string,
  now = Date.now(),
  stallRepeatMinutes = envNumber("EPIC_STALL_REPEAT_MIN", DEFAULT_STALL_REPEAT),
): SeenMark["stall"] {
  const stall = seen[key]?.stall;
  if (!stall) throw new Error(`no unresolved stall for ${key}`);
  stall.acknowledgedAt = new Date(now).toISOString();
  stall.nextAlertAt = now + Math.max(1, stallRepeatMinutes) * 60_000;
  return stall;
}
