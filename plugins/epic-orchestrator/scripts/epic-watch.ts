#!/usr/bin/env bun
/** Block until an epic worker needs the orchestrator, then print events and exit. */

import { readdirSync } from "node:fs";
import { join } from "node:path";
import { CommandError, herdr, readJson, writeJson } from "./common.ts";

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

function snapshot(
  state: string,
): [Record<string, Record<string, unknown>>, Record<string, Record<string, unknown>>] {
  const records: Record<string, Record<string, unknown>> = {};
  try {
    for (const name of readdirSync(join(state, "issues"))) {
      if (!name.endsWith(".json")) continue;
      records[name.slice(0, -5)] = readJson(join(state, "issues", name), {});
    }
  } catch {
    // empty
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

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  let stateDir: string | undefined;
  let interval = 60;
  let maxWait = 2700;
  let recheck = 300;
  let peek = false;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--state-dir") stateDir = argv[++i];
    else if (a === "--interval") interval = Number(argv[++i]);
    else if (a === "--max-wait") maxWait = Number(argv[++i]);
    else if (a === "--recheck") recheck = Number(argv[++i]);
    else if (a === "--peek") peek = true;
    else throw new Error(`unknown arg: ${a}`);
  }
  if (!stateDir) throw new Error("--state-dir is required");
  if (peek) maxWait = 0;
  const seenPath = join(stateDir, "watch-seen.json");
  const seen = readJson<Record<string, SeenMark>>(seenPath, {});
  const started = Date.now();
  for (;;) {
    const [active, statuses] = snapshot(stateDir);
    const agents = await liveAgents();
    let events: Record<string, unknown>[] = [];
    for (const [k, r] of Object.entries(active)) {
      events.push(...issueEvents(k, r, statuses[k] ?? {}, agents, seen));
    }
    const elapsed = (Date.now() - started) / 1000;
    if (
      events.length === 0 &&
      elapsed >= recheck &&
      Object.values(active).some((r) => r.stage === "awaiting_merge")
    ) {
      events.push({
        type: "recheck",
        keys: Object.entries(active)
          .filter(([, r]) => r.stage === "awaiting_merge")
          .map(([k]) => k),
      });
    }
    if (events.length === 0 && (elapsed >= maxWait || Object.keys(active).length === 0)) {
      events.push({
        type: Object.keys(active).length > 0 ? "heartbeat" : "idle-no-active-issues",
      });
    }
    if ("__error__" in agents) {
      events.push({ type: "herdr-error", error: agents.__error__ });
    }
    if (events.length > 0) {
      if (!peek) await writeJson(seenPath, seen);
      const summary: Record<string, unknown> = {};
      for (const [k, r] of Object.entries(active)) {
        const st = statuses[k] ?? {};
        const agentName = typeof r.agent === "string" ? r.agent : k;
        summary[k] = {
          stage: r.stage,
          phase: st.phase,
          pr: st.pr,
          agent: agents[agentName],
        };
      }
      process.stdout.write(JSON.stringify({ events, summary }, null, 2) + "\n");
      return;
    }
    await Bun.sleep(interval * 1000);
  }
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    process.stderr.write(String(err instanceof Error ? err.message : err) + "\n");
    process.exit(1);
  });
}
