/** Block until an epic worker needs the orchestrator, then print events and exit. */

import { readdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { snapshot, snapshotActive, type Snapshot } from "./checkpoint.ts";
import { CommandError, herdr, readJson, writeJson } from "./common.ts";
import { HOST_LIMIT, parseHostKind } from "./hosts.ts";
import { budgetLow, envNumber, ghBudget } from "./ratelimit.ts";
import type { Cooldowns } from "./route.ts";

const ACTIVE_STAGES = new Set(["running", "awaiting_merge"]);
const REPORTABLE = new Set(["ready", "needs-human", "failed"]);
const STALL_MINUTES: Record<string, number | null> = { babysit: 120, ready: null };
const DEFAULT_STALL = 45;

export function ageMinutes(stamp: string | null | undefined): number {
  if (!stamp) return 0;
  const then = Date.parse(stamp.endsWith("Z") ? stamp : stamp + "Z");
  if (Number.isNaN(then)) return 0;
  return (Date.now() - then) / 60000;
}

async function liveAgents(): Promise<Record<string, string>> {
  try {
    const agents =
      ((await herdr(["agent", "list"])).agents as
        | { name?: string; agent_status?: string }[]
        | undefined) ?? [];
    const out: Record<string, string> = {};
    for (const a of agents) {
      if (a.name) out[a.name] = a.agent_status ?? "unknown";
    }
    return out;
  } catch (err) {
    if (err instanceof CommandError) return { __error__: String(err) };
    throw err;
  }
}

type SeenMark = {
  reported?: string;
  gone?: boolean;
  blocked?: boolean;
  stalled?: string;
  limitScan?: string | undefined;
  limited?: string;
};

export function issueEvents(
  key: string,
  rec: Record<string, unknown>,
  status: Record<string, unknown>,
  agents: Record<string, string>,
  seen: Record<string, SeenMark>,
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
  const agentName = typeof rec.agent === "string" ? rec.agent : key;
  const agentState = agents[agentName];
  if (!("__error__" in agents) && agentState === undefined && rec.stage === "running") {
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
  const limit =
    phase !== undefined && phase in STALL_MINUTES ? STALL_MINUTES[phase] : DEFAULT_STALL;
  const idle = agentState === "idle" || agentState === "done";
  const stamp = updated ?? (typeof rec.last_launch === "string" ? rec.last_launch : undefined);
  if (limit && idle && ageMinutes(stamp) > limit && mark.stalled !== stamp) {
    if (stamp !== undefined) mark.stalled = stamp;
    events.push({ ...base, type: "stalled", minutes: Math.round(ageMinutes(stamp)) });
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
  const proc = Bun.spawn(
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
    { stdout: "pipe", stderr: "pipe" },
  );
  const [out, code] = await Promise.all([new Response(proc.stdout).text(), proc.exited]);
  return code === 0 ? out : "";
}

/** Put a host on cooldown so routing sends new and moved work elsewhere.
 * Returns false (and writes nothing) when `kind` names no known host, e.g. a
 * hand-edited record: one bad record must not crash the whole watcher. */
export async function coolHost(
  state: string,
  kind: string,
  reason: string,
  now = Date.now(),
): Promise<boolean> {
  const host = parseHostKind(kind);
  if (!host) return false;
  const path = join(state, "hosts.json");
  const hosts = readJson<Cooldowns>(path, {});
  const minutes = envNumber("EPIC_HOST_COOLDOWN_MIN", 60);
  hosts[host] = { until: new Date(now + minutes * 60_000).toISOString(), reason };
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
      const cooled = await coolHost(state, kind, line.slice(0, 200));
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

let lastBudgetAlert = 0;

async function budgetEvent(): Promise<Record<string, unknown> | null> {
  const b = await ghBudget();
  const low = b ? budgetLow(b) : null;
  if (!b || !low) return null;
  const reset = Math.min(b.core.reset, b.graphql.reset);
  if (lastBudgetAlert === reset) return null;
  lastBudgetAlert = reset;
  return { type: "gh-budget-low", note: low, reset: new Date(reset).toISOString() };
}

type Lock = { pid: number; started: string };

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/** One watcher per epic: a second one would double every event. */
async function takeLock(state: string): Promise<string | null> {
  const path = join(state, "watch.lock");
  const held = readJson<Lock | null>(path, null);
  if (held && held.pid !== process.pid && alive(held.pid)) {
    return `watcher already running (pid ${held.pid} since ${held.started})`;
  }
  await writeJson(path, { pid: process.pid, started: new Date().toISOString() });
  const release = () => rmSync(path, { force: true });
  process.on("exit", release);
  for (const sig of ["SIGINT", "SIGTERM", "SIGHUP"] as const) {
    process.on(sig, () => {
      release();
      process.exit(130);
    });
  }
  return null;
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

type WatchOpts = {
  stateDir: string;
  interval: number;
  maxWait: number;
  recheck: number;
  checkpointMin: number;
  peek: boolean;
};

function parseArgs(argv: string[]): WatchOpts {
  const o: WatchOpts = {
    stateDir: "",
    interval: 60,
    maxWait: 2700,
    recheck: 300,
    checkpointMin: 15,
    peek: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--state-dir") o.stateDir = argv[++i] ?? "";
    else if (a === "--interval") o.interval = Number(argv[++i]);
    else if (a === "--max-wait") o.maxWait = Number(argv[++i]);
    else if (a === "--recheck") o.recheck = Number(argv[++i]);
    else if (a === "--checkpoint") o.checkpointMin = Number(argv[++i]);
    else if (a === "--peek") o.peek = true;
    else throw new Error(`unknown arg: ${a}`);
  }
  if (!o.stateDir) throw new Error("--state-dir is required");
  if (o.peek) o.maxWait = 0;
  return o;
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
  const o = parseArgs(process.argv.slice(2));
  if (!o.peek) {
    const held = await takeLock(o.stateDir);
    if (held) {
      process.stdout.write(
        JSON.stringify({ events: [{ type: "watcher-busy", note: held }] }) + "\n",
      );
      return;
    }
  }
  const seenPath = join(o.stateDir, "watch-seen.json");
  const seen = readJson<Record<string, SeenMark>>(seenPath, {});
  const started = Date.now();
  let lastCheckpoint = 0;
  let lastBudget = 0;
  let budgetHold = false;
  const saved: Snapshot[] = [];
  for (;;) {
    const [active, statuses] = readState(o.stateDir);
    const agents = await liveAgents();
    const now = Date.now();
    if (!o.peek && o.checkpointMin > 0 && now - lastCheckpoint >= o.checkpointMin * 60_000) {
      lastCheckpoint = now;
      saved.push(...(await snapshotActive(o.stateDir)).filter((c) => c.changed));
    }
    const events: Record<string, unknown>[] = [];
    for (const [k, r] of Object.entries(active)) {
      const evs = issueEvents(k, r, statuses[k] ?? {}, agents, seen);
      // A vanished agent may have left uncommitted work; snapshot it now.
      if (!o.peek && evs.some((e) => e.type === "gone")) {
        const snap = await snapshot(typeof r.worktree === "string" ? r.worktree : "", k);
        if (snap.changed) saved.push(snap);
      }
      events.push(...evs);
    }
    if (!o.peek) events.push(...(await limitEvents(o.stateDir, active, statuses, agents, seen)));
    if (now - lastBudget >= 300_000) {
      lastBudget = now;
      const low = await budgetEvent();
      budgetHold = low !== null || (budgetHold && lastBudgetAlert > now);
      if (low) events.push(low);
    }
    const elapsed = (now - started) / 1000;
    const awaiting = Object.entries(active).filter(([, r]) => r.stage === "awaiting_merge");
    // Low GitHub budget: hold merge-gate rechecks until it resets.
    if (events.length === 0 && elapsed >= o.recheck && awaiting.length && !budgetHold) {
      events.push({ type: "recheck", keys: awaiting.map(([k]) => k) });
    }
    if (events.length === 0 && (elapsed >= o.maxWait || Object.keys(active).length === 0)) {
      events.push({ type: Object.keys(active).length > 0 ? "heartbeat" : "idle-no-active-issues" });
    }
    if ("__error__" in agents) events.push({ type: "herdr-error", error: agents.__error__ });
    if (events.length > 0) {
      if (!o.peek) await writeJson(seenPath, seen);
      const out = { events, summary: summarize(active, statuses, agents), checkpoints: saved };
      process.stdout.write(JSON.stringify(out, null, 2) + "\n");
      return;
    }
    await Bun.sleep(o.interval * 1000);
  }
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    process.stderr.write(String(err instanceof Error ? err.message : err) + "\n");
    process.exit(1);
  });
}
