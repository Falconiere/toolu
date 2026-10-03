/** Route each sub-issue to a worker host, model, and effort.
 *
 * Jev scores each issue's implementation complexity into a tier; a routing
 * table maps tier -> model/effort per host; hosts are filled by free capacity,
 * skipping any host cooling down after a usage limit. Routes persist in
 * `<state>/routes/<key>.json`, so relaunches keep their host and model. */

import { existsSync, readdirSync, realpathSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { readResourceState, resourceHome } from "../hooks/dist/epic-runtime.js";
import { runCommand } from "../hooks/dist/epic-runtime.js";
import { EPICS_HOME, readJson, writeJson } from "./common.ts";
import { occupiesSlot } from "./epic-graph.ts";
import { HOST_KINDS, hostKind, parseHostKind, type HostKind } from "./hosts.ts";

export const TIERS = ["trivial", "standard", "complex", "critical"] as const;

const LEVELS = [
  "Trivial: a docs, config, copy, or one-line fix with no design work.",
  "Standard: a contained change in one module that follows existing patterns and has clear acceptance criteria.",
  "Complex: spans several modules or needs new design, a migration, concurrency, or substantial new tests.",
  "Critical: cross-cutting architecture, security or data-loss risk, or ambiguous requirements that need deep reasoning.",
];

export type ModelChoice = { model?: string; effort?: string };
export type RoutingTable = {
  hosts: Partial<Record<HostKind, ModelChoice[]>>;
  /** Optional per-tier host preference, e.g. {"critical": ["claude"]}. */
  prefer?: Partial<Record<(typeof TIERS)[number], HostKind[]>>;
};

/** Defaults: effort rises with the tier, and the model steps up where a host
 * has a stronger one. OpenCode keeps its configured model unless
 * routing.json names one (provider ids differ per install). */
export const DEFAULT_TABLE: RoutingTable = {
  hosts: {
    claude: [
      { model: "sonnet", effort: "low" },
      { model: "sonnet", effort: "medium" },
      { model: "opus", effort: "high" },
      { model: "opus", effort: "xhigh" },
    ],
    codex: [
      { model: "gpt-6-sol", effort: "low" },
      { model: "gpt-6-sol", effort: "medium" },
      { model: "gpt-6-sol", effort: "high" },
      { model: "gpt-6-sol", effort: "xhigh" },
    ],
    cursor: [
      { model: "composer-2.5" },
      { model: "gpt-5.6-sol-high" },
      { model: "claude-opus-5-thinking-high" },
      { model: "gpt-5.6-sol-xhigh" },
    ],
    opencode: [{}, {}, {}, {}],
  },
};

export function loadTable(env: NodeJS.ProcessEnv = process.env): RoutingTable {
  const path = env.EPIC_ROUTING_FILE || join(EPICS_HOME, "routing.json");
  const user = readJson<Partial<RoutingTable>>(path, {});
  return {
    hosts: { ...DEFAULT_TABLE.hosts, ...user.hosts },
    prefer: { ...DEFAULT_TABLE.prefer, ...user.prefer },
  };
}

export type IssueEvidence = {
  key: string;
  ref: string;
  title: string;
  labels?: string[];
  excerpt?: string;
  open_blockers?: string[];
  unblocks?: number;
};

export type Score = {
  tier: number;
  score: number | null;
  confidence: number | null;
  source: string;
};

/** Deterministic fallback when Jev is unavailable. */
export function heuristicTier(i: IssueEvidence): number {
  const text = `${i.title} ${(i.labels ?? []).join(" ")}`.toLowerCase();
  if (/\b(security|architecture|breaking|data[- ]loss|migration|rewrite)\b/.test(text)) return 3;
  if (/\b(docs?|typo|readme|copy|chore|bump|config)\b/.test(text)) return 0;
  const body = i.excerpt ?? "";
  const tasks = (body.match(/^\s*[-*]\s*\[[ xX]\]/gm) ?? []).length;
  return body.length > 1200 || tasks >= 6 || (i.unblocks ?? 0) >= 3 ? 2 : 1;
}

/** Expected Jev score, nudged up: under-provisioning a hard issue costs a
 * failed run; over-provisioning an easy one costs only tokens. */
export function tierFromScore(score: number): number {
  return Math.max(0, Math.min(3, Math.round(score + 0.15)));
}

export function jevScript(env: NodeJS.ProcessEnv = process.env): string | null {
  const candidates = [
    env.EPIC_JEV,
    join(env.CLAUDE_CONFIG_DIR || join(homedir(), ".claude"), "jev", "jev.sh"),
    join(env.CODEX_HOME || join(homedir(), ".codex"), "jev", "jev.sh"),
  ];
  return candidates.find((p): p is string => !!p && existsSync(p)) ?? null;
}

/** argv prefix that runs `jev.sh`. The installed `jev.sh` is a symlink to a
 * Bun-run `.js` bundle, which bash would parse as shell; run it with Bun. Any
 * other file (a user override) runs directly, under its own shebang. */
export function jevCommand(script: string): string[] {
  let real = script;
  try {
    real = realpathSync(script);
  } catch {}
  return real.endsWith(".js") ? [process.execPath, script] : [script];
}

export function jevQuestions(issues: IssueEvidence[]): Record<string, unknown> {
  return Object.fromEntries(
    issues.map((i) => [
      i.key,
      {
        type: "score",
        instructions:
          `How much implementation complexity does \`issues.${i.key}\` carry for one coding agent? ` +
          "Judge from its title, labels, description excerpt, and dependency counts only.",
        criteria: LEVELS,
      },
    ]),
  );
}

async function jevScores(issues: IssueEvidence[]): Promise<Record<string, Score> | string> {
  const jev = jevScript();
  if (!jev) return "jev.sh not installed";
  if (!process.env.TYPESAFE_API_KEY) return "TYPESAFE_API_KEY not set";
  const state = {
    issues: Object.fromEntries(
      issues.map((i) => [
        i.key,
        {
          title: i.title,
          labels: i.labels ?? [],
          description_excerpt: i.excerpt ?? "",
          open_blockers: (i.open_blockers ?? []).length,
          unblocks: i.unblocks ?? 0,
        },
      ]),
    ),
  };
  const result = await runCommand([...jevCommand(jev), "ask", "-", "-s", JSON.stringify(state)], {
    stdin: JSON.stringify(jevQuestions(issues)),
    timeoutMs: 30_000,
  });
  const { stdout: out, stderr: err, exitCode: code } = result;
  if (code !== 0 || result.timedOut || result.truncated)
    return `jev failed (${code}): ${(err || out).trim().slice(0, 200)}`;
  const answers = JSON.parse(out) as Record<string, { score?: number; confidence?: number }>;
  const scores: Record<string, Score> = {};
  for (const i of issues) {
    const a = answers[i.key];
    if (typeof a?.score !== "number") continue;
    scores[i.key] = {
      tier: tierFromScore(a.score),
      score: a.score,
      confidence: a.confidence ?? null,
      source: "jev",
    };
  }
  return scores;
}

export type HostPool = { kind: HostKind; cap: number }[];

/** `claude:2,codex:1` -> pool; a bare host gets `defaultCap`. */
export function parseHosts(spec: string, defaultCap: number): HostPool {
  const seen = new Set<HostKind>();
  const pool = spec
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
    .map((s) => {
      const [name, raw, extra] = s.split(":");
      const kind = hostKind(name ?? "");
      const cap = raw === undefined ? defaultCap : Number(raw);
      if (extra !== undefined || !Number.isSafeInteger(cap) || cap <= 0 || seen.has(kind))
        throw new Error(`invalid host capacity: ${s}`);
      seen.add(kind);
      return { kind, cap };
    });
  if (!pool.length) throw new Error("host pool is empty");
  return pool;
}

export type Cooldowns = Partial<Record<HostKind, { until: string; reason?: string }>>;

export function coolingHosts(c: Cooldowns, now = Date.now()): Set<HostKind> {
  return new Set(
    HOST_KINDS.filter((k) => {
      const until = c[k]?.until;
      return until !== undefined && Date.parse(until) > now;
    }),
  );
}

/** Host with the most free capacity; ties keep pool order. Preferred hosts
 * for the tier win when they have room. */
export function pickHost(
  pool: HostPool,
  used: Partial<Record<HostKind, number>>,
  cooling: Set<HostKind>,
  prefer: HostKind[] = [],
): HostKind | null {
  const open = pool.filter((h) => !cooling.has(h.kind) && (used[h.kind] ?? 0) < h.cap);
  const preferred = open.filter((h) => prefer.includes(h.kind));
  const candidates = preferred.length ? preferred : open;
  let best: { kind: HostKind; free: number } | null = null;
  for (const h of candidates) {
    const free = h.cap - (used[h.kind] ?? 0);
    if (!best || free > best.free) best = { kind: h.kind, free };
  }
  return best?.kind ?? null;
}

export type Route = {
  key: string;
  ref: string;
  tier: (typeof TIERS)[number];
  score: number | null;
  confidence: number | null;
  source: string;
  host: HostKind | null;
  model: string | null;
  effort: string | null;
  decided_at: string;
};

type Graph = {
  state_dir: string;
  max_parallel?: number;
  launch_batch: string[];
  issues: (IssueEvidence & { status?: string })[];
};

function liveHosts(state: string): Map<string, HostKind> {
  const used = new Map<string, HostKind>();
  let names: string[] = [];
  try {
    names = readdirSync(join(state, "issues"));
  } catch (err: unknown) {
    const code = err && typeof err === "object" && "code" in err ? err.code : undefined;
    if (code === "ENOENT") return used;
    throw err;
  }
  for (const name of names) {
    const rec = readJson<{ stage?: string; kind?: string }>(join(state, "issues", name), {});
    if (!occupiesSlot(rec.stage)) continue;
    // A record naming no known host occupies no host's capacity.
    const kind = parseHostKind(rec.kind ?? "claude");
    if (kind) used.set(name.replace(/\.json$/, ""), kind);
  }
  return used;
}

export async function routeIssues(
  graph: Graph,
  wanted: string[],
  pool: HostPool,
  opts: { jev: boolean; reroute: boolean; table: RoutingTable; resourceRoot?: string },
): Promise<{ routes: Route[]; note: string | null }> {
  const dir = join(graph.state_dir, "routes");
  // Separate maps so a ref can never be shadowed by another issue's key.
  const byRef = new Map(graph.issues.map((i) => [i.ref, i] as const));
  const byKey = new Map(graph.issues.map((i) => [i.key, i] as const));
  const targets = [
    ...new Set(
      wanted.map((w) => {
        const i = byRef.get(w) ?? byKey.get(w);
        if (!i) throw new Error(`${w} is not in the graph`);
        return i;
      }),
    ),
  ];
  const existing = new Map(
    targets.map((i) => [i.key, readJson<Route | null>(join(dir, `${i.key}.json`), null)]),
  );
  const toScore = targets.filter((i) => !existing.get(i.key));
  let note: string | null = null;
  let scores: Record<string, Score> = {};
  if (toScore.length && opts.jev) {
    const res = await jevScores(toScore);
    if (typeof res === "string") note = `jev unavailable (${res}); heuristic tiers used`;
    else scores = res;
  }
  const active = liveHosts(graph.state_dir);
  const used: Partial<Record<HostKind, number>> = {};
  for (const host of active.values()) used[host] = (used[host] ?? 0) + 1;
  const cooling = coolingHosts(readJson<Cooldowns>(join(graph.state_dir, "hosts.json"), {}));
  if (opts.resourceRoot) {
    const shared = readResourceState(opts.resourceRoot);
    for (const lease of shared.leases) {
      if (lease.type !== "agent") continue;
      for (const host of new Set([lease.host, lease.pendingHost])) {
        const kind = parseHostKind(host ?? "");
        if (!kind || (lease.stateDir === graph.state_dir && active.get(lease.key) === kind))
          continue;
        used[kind] = (used[kind] ?? 0) + 1;
      }
    }
    for (const [host, c] of Object.entries(shared.cooldowns)) {
      const kind = parseHostKind(host);
      if (kind && c.until > Date.now()) cooling.add(kind);
    }
  }
  await writeJson(join(graph.state_dir, "pool.json"), pool);
  const routes: Route[] = [];
  const now = new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
  for (const i of targets) {
    const prev = existing.get(i.key);
    const live = active.get(i.key);
    const keepHost =
      prev?.host &&
      !opts.reroute &&
      !cooling.has(prev.host) &&
      pool.some((h) => h.kind === prev.host && ((used[h.kind] ?? 0) < h.cap || live === h.kind));
    const s = prev
      ? {
          tier: TIERS.indexOf(prev.tier),
          score: prev.score,
          confidence: prev.confidence,
          source: prev.source,
        }
      : (scores[i.key] ?? {
          tier: heuristicTier(i),
          score: null,
          confidence: null,
          source: "heuristic",
        });
    const tierName = TIERS[s.tier] ?? "standard";
    let host: HostKind | null = keepHost ? (prev?.host ?? null) : null;
    if (!host) {
      host = pickHost(pool, used, cooling, opts.table.prefer?.[tierName]);
    }
    if (host && live !== host) used[host] = (used[host] ?? 0) + 1;
    const choice = host ? (opts.table.hosts[host]?.[s.tier] ?? {}) : {};
    const route: Route = {
      key: i.key,
      ref: i.ref,
      tier: tierName,
      score: s.score,
      confidence: s.confidence,
      source: s.source,
      host,
      model: choice.model ?? null,
      effort: choice.effort ?? null,
      decided_at: prev && keepHost ? prev.decided_at : now,
    };
    await writeJson(join(dir, `${i.key}.json`), route);
    routes.push(route);
  }
  return { routes, note };
}

function renderRoutes(routes: Route[], note: string | null): string {
  const lines = [
    `${"issue".padEnd(28)} ${"tier".padEnd(9)} ${"score".padStart(5)} ${"host".padEnd(9)} ${"model".padEnd(28)} effort  source`,
  ];
  for (const r of routes) {
    lines.push(
      `${r.ref.padEnd(28)} ${r.tier.padEnd(9)} ${(r.score?.toFixed(2) ?? "-").padStart(5)} ${(r.host ?? "NONE").padEnd(9)} ${(r.model ?? "(host default)").padEnd(28)} ${(r.effort ?? "-").padEnd(7)} ${r.source}`,
    );
  }
  if (routes.some((r) => !r.host)) lines.push("NONE = no host has free capacity; wait for a slot.");
  if (note) lines.push(note);
  return lines.join("\n");
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  let graphPath: string | undefined;
  let hosts = "claude";
  const issues: string[] = [];
  let jev = true;
  let reroute = false;
  let asJson = false;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--graph") graphPath = argv[++i];
    else if (a === "--hosts") hosts = argv[++i] ?? hosts;
    else if (a === "--issue") issues.push(argv[++i] ?? "");
    else if (a === "--no-jev") jev = false;
    else if (a === "--reroute") reroute = true;
    else if (a === "--json") asJson = true;
    else throw new Error(`unknown arg: ${a}`);
  }
  if (!graphPath) {
    throw new Error(
      "usage: route.ts --graph GRAPH.json [--hosts claude:2,codex:1] [--issue REF]... [--no-jev] [--reroute] [--json]",
    );
  }
  const graph = readJson<Graph | null>(graphPath, null);
  if (!graph) throw new Error(`cannot read graph ${graphPath}`);
  const pool = parseHosts(hosts, graph.max_parallel ?? 3);
  const wanted = issues.length ? issues : graph.launch_batch;
  const { routes, note } = await routeIssues(graph, wanted, pool, {
    jev,
    reroute,
    table: loadTable(),
    resourceRoot: resourceHome(),
  });
  process.stdout.write(
    (asJson ? JSON.stringify({ routes, note }, null, 2) : renderRoutes(routes, note)) + "\n",
  );
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    process.stderr.write(String(err instanceof Error ? err.message : err) + "\n");
    process.exit(1);
  });
}
