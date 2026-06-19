// Machine-wide dashboard config: which project roots to scan and the thresholds
// that drive discovery, staleness, and refresh. Pure + cwd-independent. Never
// throws — a missing file yields documented defaults, malformed JSON yields
// defaults plus a warning, so the server can always start.

import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

/** Tunables the dashboard reads at startup; see docs spec for semantics. */
export interface DashboardConfig {
  /** Base dirs to scan for plan-ledger JSON under each repo's .claude/tmp (absolute, ~ expanded). */
  roots: string[];
  /** Max walk depth under each root. */
  scanDepth: number;
  /** A project is "active" if its ledger mtime is within this many hours. */
  activeWithinHours: number;
  /** Plan-lane: running step older than this (seconds) reads stuck. */
  stuckThresholdSeconds: number;
  /** Activity-lane: unpaired tool_use older than this (seconds) reads stale, not running. */
  agentStuckSeconds: number;
  /** Refresh cadence in ms. */
  pollMs: number;
  /** Listen port; 0 = ephemeral. */
  port: number;
  /** Open the browser on start. */
  open: boolean;
}

/** The defaults applied when a key is absent or the file is missing/corrupt. */
export const DEFAULT_CONFIG: DashboardConfig = {
  roots: [],
  scanDepth: 3,
  activeWithinHours: 12,
  stuckThresholdSeconds: 300,
  agentStuckSeconds: 600,
  pollMs: 1500,
  port: 0,
  open: false,
};

/** Default config location, shared by the loader and the config CLI. */
export const DEFAULT_CONFIG_PATH: string = join(
  process.env.XDG_CONFIG_HOME && process.env.XDG_CONFIG_HOME.length > 0
    ? process.env.XDG_CONFIG_HOME
    : join(homedir(), ".config"),
  "toolu",
  "dashboard.json",
);

/** Expand a leading `~`, `$HOME`, and `${HOME}` to an absolute home path. */
export function expandHome(p: string): string {
  const home = homedir();
  if (p === "~") return home;
  if (p.startsWith("~/")) return join(home, p.slice(2));
  return p.replace(/\$\{HOME\}|\$HOME(?![A-Za-z0-9_])/g, home);
}

/** Coerce one parsed value to the type of its default, falling back on mismatch. */
function pick<T>(value: unknown, fallback: T): T {
  if (typeof value === typeof fallback && typeof value !== "object") return value as T;
  return fallback;
}

/** Pick a number, then floor it at `min` so out-of-range config can't misbehave
 *  (e.g. a negative pollMs would busy-loop setInterval; negative scanDepth no-ops). */
function num(value: unknown, fallback: number, min: number): number {
  return Math.max(min, pick(value, fallback));
}

/** Merge a parsed object over DEFAULT_CONFIG with per-field type guards. */
function merge(parsed: Record<string, unknown>): DashboardConfig {
  const rawRoots = parsed.roots;
  const roots = Array.isArray(rawRoots)
    ? rawRoots.filter((r): r is string => typeof r === "string").map(expandHome)
    : DEFAULT_CONFIG.roots;
  return {
    roots,
    scanDepth: num(parsed.scanDepth, DEFAULT_CONFIG.scanDepth, 0),
    activeWithinHours: num(parsed.activeWithinHours, DEFAULT_CONFIG.activeWithinHours, 0),
    stuckThresholdSeconds: num(parsed.stuckThresholdSeconds, DEFAULT_CONFIG.stuckThresholdSeconds, 0),
    agentStuckSeconds: num(parsed.agentStuckSeconds, DEFAULT_CONFIG.agentStuckSeconds, 0),
    pollMs: num(parsed.pollMs, DEFAULT_CONFIG.pollMs, 1),
    port: num(parsed.port, DEFAULT_CONFIG.port, 0),
    open: pick(parsed.open, DEFAULT_CONFIG.open),
  };
}

/** Load config from disk, fill defaults, expand ~. Never throws: a missing file
 *  returns defaults with no warning; malformed JSON returns defaults + a warning. */
export function loadConfig(path: string = DEFAULT_CONFIG_PATH): {
  config: DashboardConfig;
  warning?: string;
} {
  if (!existsSync(path)) return { config: { ...DEFAULT_CONFIG } };
  let raw: string;
  try {
    raw = readFileSync(path, "utf8");
  } catch (err) {
    return { config: { ...DEFAULT_CONFIG }, warning: `cannot read config ${path}: ${String(err)}` };
  }
  if (raw.trim().length === 0) return { config: { ...DEFAULT_CONFIG } };
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { config: { ...DEFAULT_CONFIG }, warning: `config json is malformed at ${path}` };
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    return { config: { ...DEFAULT_CONFIG }, warning: `config at ${path} is not a JSON object` };
  }
  return { config: merge(parsed as Record<string, unknown>) };
}
