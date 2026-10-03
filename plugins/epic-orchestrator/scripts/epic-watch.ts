/** Block until an epic worker needs the orchestrator, then print events and exit. */

import { readdirSync } from "node:fs";
import { join } from "node:path";
import { runCommand } from "../hooks/dist/epic-runtime.js";
import {
  coolResourceHost,
  refreshPressure,
  resourceHome,
  writeJsonAtomic,
} from "../hooks/dist/epic-runtime.js";
import { snapshot, type Snapshot } from "./checkpoint.ts";
import { CommandError, readJson, writeJson } from "./common.ts";
import { HOST_LIMIT, parseHostKind } from "./hosts.ts";
import { budgetLow, envNumber, ghBudget } from "./ratelimit.ts";
import type { Cooldowns } from "./route.ts";
import { takeWatchLock } from "./watch/ownership.ts";
import {
  acknowledgeStall,
  parseWatchArgs,
  readWatchRuntime,
  type SeenMark,
  type WatchRuntime,
} from "./watch/state.ts";
export {
  acknowledgeStall,
  initialWatchRuntime,
  parseWatchArgs,
  readWatchRuntime,
} from "./watch/state.ts";
export { takeWatchLock } from "./watch/ownership.ts";

const ACTIVE_STAGES = new Set(["running", "awaiting_merge"]);
const REPORTABLE = new Set(["ready", "needs-human", "failed"]);
const STALL_MINUTES: Record<string, number | null> = { babysit: 120, ready: null };
const DEFAULT_STALL = 45;
const DEFAULT_STALL_REPEAT = 30;
const BUDGET_INTERVAL_MS = 300_000;
const DEFAULT_HERDR_BACKOFF_MS = 30_000;
const DEFAULT_HERDR_BACKOFF_MAX_MS = 300_000;
const HERDR_TIMEOUT_MS = 30_000;
const PANE_TIMEOUT_MS = 15_000;

export function ageMinutes(stamp: string | null | undefined, now = Date.now()): number {
  if (!stamp) return 0;
  const zoned = /(?:Z|[+-]\d{2}:\d{2})$/i.test(stamp) ? stamp : `${stamp}Z`;
  const then = Date.parse(zoned);
  if (Number.isNaN(then)) return 0;
  return (now - then) / 60000;
}

async function liveAgents(): Promise<Record<string, string>> {
  const result = await runCommand(["herdr", "agent", "list"], {
    timeoutMs: HERDR_TIMEOUT_MS,
    maxOutputBytes: 512 * 1024,
  });
  const text = result.stdout.trim() || result.stderr.trim();
  if (result.exitCode !== 0 || result.timedOut || result.cancelled || result.truncated) {
    throw new CommandError(
      `herdr agent list failed (${result.exitCode}${result.timedOut ? ", timed out" : ""}): ${text.slice(0, 300)}`,
    );
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new CommandError(`herdr agent list: non-JSON output: ${text.slice(0, 300)}`);
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new CommandError("herdr agent list: expected object JSON");
  }
  const object = parsed as Record<string, unknown>;
  const resultObject =
    object.result && typeof object.result === "object" && !Array.isArray(object.result)
      ? (object.result as Record<string, unknown>)
      : object;
  if (resultObject.error) {
    const message =
      typeof resultObject.error === "string"
        ? resultObject.error
        : (JSON.stringify(resultObject.error) ?? "unknown error");
    throw new CommandError(`herdr agent list: ${message}`);
  }
  const listed = Array.isArray(resultObject.agents) ? resultObject.agents : [];
  const out: Record<string, string> = {};
  for (const value of listed) {
    if (!value || typeof value !== "object" || Array.isArray(value)) continue;
    const agent = value as Record<string, unknown>;
    if (typeof agent.name === "string") {
      out[agent.name] = typeof agent.agent_status === "string" ? agent.agent_status : "unknown";
    }
  }
  return out;
}

export function issueEvents(
  key: string,
  rec: Record<string, unknown>,
  status: Record<string, unknown>,
  agents: Record<string, string>,
  seen: Record<string, SeenMark>,
  now = Date.now(),
  stallRepeatMinutes = envNumber("EPIC_STALL_REPEAT_MIN", DEFAULT_STALL_REPEAT),
): Record<string, unknown>[] {
  const events: Record<string, unknown>[] = [];
  const mark = (seen[key] ??= {});
  const phase = status.phase as string | undefined;
  const updated = status.updated_at as string | undefined;
  const base = {
    key,
    ref: rec.ref,
    phase,
    pr: status.pr,
    note: status.note,
  };
  if (phase && REPORTABLE.has(phase) && mark.reported !== updated) {
    if (updated !== undefined) mark.reported = updated;
    events.push({ ...base, type: phase });
  }
  if ("__error__" in agents) return events;
  const agentName = typeof rec.agent === "string" ? rec.agent : key;
  const agentState = agents[agentName];
  if (agentState === undefined && rec.stage === "running") {
    if (!mark.gone) {
      mark.gone = true;
      events.push({ ...base, type: "gone" });
    }
  } else {
    mark.gone = false;
  }
  if (agentState === "blocked") {
    if (!mark.blocked) {
      mark.blocked = true;
      events.push({ ...base, type: "blocked" });
    }
  } else {
    mark.blocked = false;
  }
  const configuredLimit = phase === undefined ? undefined : STALL_MINUTES[phase];
  const limit = configuredLimit === undefined ? DEFAULT_STALL : configuredLimit;
  const parked = phase === "ready" || phase === "needs-human";
  const supervised = agentState === "working" || agentState === "idle" || agentState === "done";
  const stamp = updated ?? (typeof rec.last_launch === "string" ? rec.last_launch : undefined);
  const stale = !parked && limit !== null && supervised && ageMinutes(stamp, now) > limit;
  if (stale && stamp !== undefined) {
    const repeatMs = Math.max(1, stallRepeatMinutes) * 60_000;
    const episode = mark.stall?.stamp === stamp ? mark.stall : undefined;
    if (!episode || now >= episode.nextAlertAt) {
      const alerts = (episode?.alerts ?? 0) + 1;
      mark.stall = { stamp, nextAlertAt: now + repeatMs, alerts };
      mark.stalled = stamp;
      events.push({
        ...base,
        type: "stalled",
        minutes: Math.round(ageMinutes(stamp, now)),
        occurrence: alerts,
        acknowledged_at: episode?.acknowledgedAt,
      });
    }
  } else {
    delete mark.stall;
    delete mark.stalled;
  }
  return events;
}

/** Last lines of an agent's pane that name a provider usage/rate limit. */
export function limitLine(tail: string): string | null {
  const lines = tail
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
  return lines.reverse().find((l) => HOST_LIMIT.test(l)) ?? null;
}

async function paneTail(agent: string): Promise<string> {
  const result = await runCommand(
    [
      "herdr",
      "agent",
      "read",
      agent,
      "--source",
      "recent-unwrapped",
      "--lines",
      "15",
      "--format",
      "text",
    ],
    { timeoutMs: PANE_TIMEOUT_MS, maxOutputBytes: 128 * 1024 },
  );
  return result.exitCode === 0 && !result.timedOut && !result.cancelled && !result.truncated
    ? result.stdout
    : "";
}

/** Put a host on cooldown so routing sends new and moved work elsewhere.
 * Returns false (and writes nothing) when `kind` names no known host, e.g. a
 * hand-edited record: one bad record must not crash the whole watcher. */
export async function coolHost(
  state: string,
  kind: string,
  reason: string,
  now = Date.now(),
  resourceRoot?: string,
): Promise<boolean> {
  const host = parseHostKind(kind);
  if (!host) return false;
  const path = join(state, "hosts.json");
  const hosts = readJson<Cooldowns>(path, {});
  const minutes = envNumber("EPIC_HOST_COOLDOWN_MIN", 60);
  const until = now + minutes * 60_000;
  if (resourceRoot !== undefined) await coolResourceHost(resourceRoot, host, until, reason);
  hosts[host] = { until: new Date(until).toISOString(), reason };
  await writeJson(path, hosts);
  return true;
}

/** Idle workers whose status went quiet get their pane tail scanned for a
 * provider limit; a worker may also report `failed` with a rate-limited note. */
export async function limitEvents(
  state: string,
  active: Record<string, Record<string, unknown>>,
  statuses: Record<string, Record<string, unknown>>,
  agents: Record<string, string>,
  seen: Record<string, SeenMark>,
  resourceRoot?: string,
): Promise<Record<string, unknown>[]> {
  const events: Record<string, unknown>[] = [];
  const quietMin = envNumber("EPIC_LIMIT_SCAN_MIN", 5);
  for (const [key, rec] of Object.entries(active)) {
    const st = statuses[key] ?? {};
    const mark = (seen[key] ??= {});
    const agent = typeof rec.agent === "string" ? rec.agent : key;
    const kind = typeof rec.kind === "string" ? rec.kind : "claude";
    const stamp = (st.updated_at as string | undefined) ?? (rec.last_launch as string | undefined);
    const note = typeof st.note === "string" ? st.note : "";
    let line: string | null = st.phase === "failed" && /^rate-limited/i.test(note) ? note : null;
    // Workers parked on purpose (ready, needs-human) are idle by design.
    const parked = st.phase === "ready" || st.phase === "needs-human";
    const idle = !parked && ["idle", "done", "blocked"].includes(agents[agent] ?? "");
    if (!line && idle && ageMinutes(stamp) >= quietMin && mark.limitScan !== stamp) {
      mark.limitScan = stamp;
      line = limitLine(await paneTail(agent));
    }
    if (line && mark.limited !== stamp) {
      if (stamp !== undefined) mark.limited = stamp;
      const cooled = await coolHost(state, kind, line.slice(0, 200), Date.now(), resourceRoot);
      // Still report it: an unknown host needs the orchestrator's eyes more.
      events.push({
        key,
        ref: rec.ref,
        type: "host-limited",
        host: kind,
        cooldown: cooled,
        note: line.slice(0, 200),
      });
    }
  }
  return events;
}

async function budgetEvent(runtime: WatchRuntime): Promise<Record<string, unknown> | null> {
  const b = await ghBudget();
  const low = b ? budgetLow(b) : null;
  if (!b) return null;
  if (!low) {
    runtime.budgetHoldUntil = 0;
    return null;
  }
  const reset = Math.min(b.core.reset, b.graphql.reset);
  runtime.budgetHoldUntil = reset;
  if (runtime.budgetAlertReset === reset) return null;
  runtime.budgetAlertReset = reset;
  return { type: "gh-budget-low", note: low, reset: new Date(reset).toISOString() };
}

function readState(
  state: string,
): [Record<string, Record<string, unknown>>, Record<string, Record<string, unknown>>] {
  const records: Record<string, Record<string, unknown>> = {};
  try {
    for (const name of readdirSync(join(state, "issues"))) {
      if (!name.endsWith(".json")) continue;
      records[name.slice(0, -5)] = readJson(join(state, "issues", name), {});
    }
  } catch (err: unknown) {
    const code = err && typeof err === "object" && "code" in err ? err.code : undefined;
    if (code !== "ENOENT") throw err;
  }
  const active: Record<string, Record<string, unknown>> = {};
  for (const [k, r] of Object.entries(records)) {
    if (typeof r.stage === "string" && ACTIVE_STAGES.has(r.stage)) active[k] = r;
  }
  const statuses: Record<string, Record<string, unknown>> = {};
  for (const k of Object.keys(active)) {
    statuses[k] = readJson(join(state, "status", `${k}.json`), {});
  }
  return [active, statuses];
}

type AgentPoll = {
  agents: Record<string, string>;
  error?: string;
};

function positiveEnvMs(name: string, fallbackMs: number): number {
  const seconds = Number(process.env[name]);
  return Number.isFinite(seconds) && seconds > 0 ? seconds * 1000 : fallbackMs;
}

async function pollAgents(runtime: WatchRuntime, now: number, peek: boolean): Promise<AgentPoll> {
  if (!peek && now < runtime.herdrRetryAt) {
    return { agents: { __error__: `retry after ${new Date(runtime.herdrRetryAt).toISOString()}` } };
  }
  try {
    const agents = await liveAgents();
    if (!peek) {
      runtime.herdrFailures = 0;
      runtime.herdrRetryAt = 0;
    }
    return { agents };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (!peek) {
      runtime.herdrFailures += 1;
      const base = positiveEnvMs("EPIC_HERDR_BACKOFF_S", DEFAULT_HERDR_BACKOFF_MS);
      const cap = positiveEnvMs("EPIC_HERDR_BACKOFF_MAX_S", DEFAULT_HERDR_BACKOFF_MAX_MS);
      const exponent = Math.min(runtime.herdrFailures - 1, 10);
      runtime.herdrRetryAt = now + Math.min(cap, base * 2 ** exponent);
    }
    return { agents: { __error__: message }, error: message };
  }
}

function checkpointBatchSize(): number {
  const value = Number(process.env.EPIC_CHECKPOINT_BATCH);
  return Number.isSafeInteger(value) && value > 0 ? value : 4;
}

function fillCheckpointQueue(
  runtime: WatchRuntime,
  active: Record<string, Record<string, unknown>>,
): void {
  if (runtime.checkpointQueue.length === 0) {
    runtime.checkpointQueue = Object.keys(active).sort();
  }
}

function takeCheckpointBatch(
  runtime: WatchRuntime,
  active: Record<string, Record<string, unknown>>,
): [string, Record<string, unknown>][] {
  const selected: [string, Record<string, unknown>][] = [];
  const worktrees = new Set<string>();
  while (runtime.checkpointQueue.length > 0 && selected.length < checkpointBatchSize()) {
    const key = runtime.checkpointQueue.shift();
    if (key === undefined) break;
    const rec = active[key];
    const worktree = typeof rec?.worktree === "string" ? rec.worktree : "";
    if (!rec || worktree === "" || worktrees.has(worktree)) continue;
    worktrees.add(worktree);
    selected.push([key, rec]);
  }
  return selected;
}

async function runSnapshots(requests: [string, Record<string, unknown>][]): Promise<Snapshot[]> {
  const saved: Snapshot[] = [];
  const worktrees = new Set<string>();
  for (const [key, rec] of requests) {
    const worktree = typeof rec.worktree === "string" ? rec.worktree : "";
    if (worktree === "" || worktrees.has(worktree)) continue;
    worktrees.add(worktree);
    const result = await snapshot(worktree, key);
    if (result.changed || (result.skipped && result.skipped !== "nothing at risk")) {
      saved.push(result);
    }
  }
  return saved;
}

function summarize(
  active: Record<string, Record<string, unknown>>,
  statuses: Record<string, Record<string, unknown>>,
  agents: Record<string, string>,
): Record<string, unknown> {
  const summary: Record<string, unknown> = {};
  for (const [k, r] of Object.entries(active)) {
    const st = statuses[k] ?? {};
    const agentName = typeof r.agent === "string" ? r.agent : k;
    summary[k] = {
      stage: r.stage,
      phase: st.phase,
      pr: st.pr,
      host: r.kind,
      agent: agents[agentName],
    };
  }
  return summary;
}

async function main(): Promise<void> {
  const o = parseWatchArgs(process.argv.slice(2));
  const lock = o.peek ? undefined : await takeWatchLock(o.stateDir);
  if (!o.peek && !lock) {
    process.stdout.write(
      `${JSON.stringify({ events: [{ type: "watcher-busy", note: "watcher already running" }] })}\n`,
    );
    return;
  }
  const release = (): void => lock?.release();
  process.once("exit", release);
  for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"] as const) {
    process.once(signal, () => {
      release();
      process.exit(130);
    });
  }
  const seenPath = join(o.stateDir, "watch-seen.json");
  const runtimePath = join(o.stateDir, "watch-state.json");
  const seen = readJson<Record<string, SeenMark>>(seenPath, {});
  const started = Date.now();
  const resources = resourceHome();
  const runtime = readWatchRuntime(runtimePath, started);
  const saved: Snapshot[] = [];
  try {
    if (o.ack !== undefined) {
      const stall = acknowledgeStall(seen, o.ack, started);
      writeJsonAtomic(seenPath, seen);
      process.stdout.write(`${JSON.stringify({ acknowledged: o.ack, stall }, null, 2)}\n`);
      return;
    }
    for (;;) {
      const [active, statuses] = readState(o.stateDir);
      const now = Date.now();
      if (!o.peek) await refreshPressure(resources);
      const polled = await pollAgents(runtime, now, o.peek);
      if (!o.peek) writeJsonAtomic(runtimePath, runtime);

      // Inspect supervision events before optional snapshots or budget queries.
      const events: Record<string, unknown>[] = [];
      const gone: [string, Record<string, unknown>][] = [];
      for (const [key, rec] of Object.entries(active)) {
        const issue = issueEvents(key, rec, statuses[key] ?? {}, polled.agents, seen, now);
        if (issue.some((event) => event.type === "gone")) gone.push([key, rec]);
        events.push(...issue);
      }
      if (polled.error) {
        events.push({
          type: "herdr-error",
          error: polled.error,
          retry_at: o.peek ? undefined : new Date(runtime.herdrRetryAt).toISOString(),
        });
      }

      if (!o.peek && events.length === 0) {
        events.push(
          ...(await limitEvents(o.stateDir, active, statuses, polled.agents, seen, resources)),
        );
      }

      if (!o.peek && events.length === 0 && now >= runtime.nextBudgetAt) {
        runtime.nextBudgetAt = now + BUDGET_INTERVAL_MS;
        writeJsonAtomic(runtimePath, runtime);
        const low = await budgetEvent(runtime);
        if (low) events.push(low);
        writeJsonAtomic(runtimePath, runtime);
      }

      if (!o.peek && gone.length > 0) {
        saved.push(...(await runSnapshots(gone)));
        for (const [key] of gone) {
          runtime.checkpointQueue = runtime.checkpointQueue.filter((queued) => queued !== key);
        }
      }

      if (
        !o.peek &&
        events.length === 0 &&
        o.checkpointMin > 0 &&
        now >= runtime.nextCheckpointAt
      ) {
        fillCheckpointQueue(runtime, active);
        saved.push(...(await runSnapshots(takeCheckpointBatch(runtime, active))));
        if (runtime.checkpointQueue.length === 0) {
          runtime.nextCheckpointAt = now + o.checkpointMin * 60_000;
        }
      }

      if (!o.peek) writeJsonAtomic(runtimePath, runtime);
      const elapsed = (Date.now() - started) / 1000;
      const awaiting = Object.entries(active).filter(([, rec]) => rec.stage === "awaiting_merge");
      const budgetHold = runtime.budgetHoldUntil > Date.now();
      if (events.length === 0 && elapsed >= o.recheck && awaiting.length && !budgetHold) {
        events.push({ type: "recheck", keys: awaiting.map(([key]) => key) });
      }
      if (events.length === 0 && (elapsed >= o.maxWait || Object.keys(active).length === 0)) {
        events.push({
          type: Object.keys(active).length > 0 ? "heartbeat" : "idle-no-active-issues",
        });
      }
      if (events.length > 0) {
        if (!o.peek) writeJsonAtomic(seenPath, seen);
        const out = {
          events,
          summary: summarize(active, statuses, polled.agents),
          checkpoints: saved,
        };
        process.stdout.write(`${JSON.stringify(out, null, 2)}\n`);
        return;
      }
      const retryWait = runtime.herdrRetryAt > Date.now() ? runtime.herdrRetryAt - Date.now() : 0;
      const waitMs = retryWait > 0 ? Math.min(o.interval * 1000, retryWait) : o.interval * 1000;
      await Bun.sleep(waitMs);
    }
  } finally {
    release();
  }
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    process.stderr.write(String(err instanceof Error ? err.message : err) + "\n");
    process.exit(1);
  });
}
