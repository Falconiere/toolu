/**
 * Transient-state sweeper for `<repo>/<host dir>/tmp` (#255), a port of
 * `toolu_sweep_state`. It runs once per session and reclaims only state that
 * is provably spent:
 *
 * - `push-review/`, `plan-ledger/`, `docs-sync/`: per-branch `*.json`,
 *   dropped when the branch is merged into base, gone, or older than the TTL.
 *   The current branch is never touched.
 * - `quality-gate-status.json`: dropped when passing, or when failing only
 *   about files that no longer exist. A failure about a live file survives
 *   at any age. An unrecognized document is kept: it is not ours to judge.
 * - `telemetry/*.jsonl`: trimmed to the retention window, deleted only when
 *   nothing is left. A file with an unparseable line is left as it is.
 *
 * Best effort throughout: failures warn (`toolu-sweep: ...`) and it never throws.
 */
import { existsSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { basename, join } from "node:path";
import { isJsonObject, loadConfig, type LoadedConfig } from "../config/config-load.ts";
import { enabled, section } from "../config/config-read.ts";
import { projectRoot, projectStateRoot } from "../host/host-roots.ts";
import { GLOBAL_GATE_KEY, readGateFile } from "./gate-file.ts";
import { baseBranch, branchSlug, branchSlugs, currentBranch, hasGit } from "./state-git.ts";
import {
  compareJqStrings,
  isoSeconds,
  stderrWarn,
  toJqJson,
  withLock,
  writeAtomic,
  type StateOptions,
  type Warn,
} from "./state-io.ts";

export const SWEEP_DEFAULT_TTL_HOURS = 24;
export const SWEEP_DEFAULT_RETENTION_DAYS = 7;
export const SWEEP_BRANCH_DIRS = ["push-review", "plan-ledger", "docs-sync"] as const;

const HOUR_MS = 3_600_000;
const DAY_MS = 24 * HOUR_MS;

/** `gates.<key>` when it is a number > 0 (floored), else `fallback`: the lenient threshold reading. */
function positiveGate(config: LoadedConfig, key: string, fallback: number): number {
  const value = section(config, "gates")?.[key];
  return typeof value === "number" && value > 0 ? Math.floor(value) : fallback;
}

/** Non-dot regular files in `dir` ending in `suffix` (a bash `*.suffix` glob plus `[ -f ]`). */
function globFiles(dir: string, suffix: string): string[] {
  if (!existsSync(dir) || !statSync(dir).isDirectory()) return [];
  return readdirSync(dir)
    .filter((name) => name.endsWith(suffix) && !name.startsWith("."))
    .map((name) => join(dir, name))
    .filter((file) => statSync(file, { throwIfNoEntry: false })?.isFile() === true);
}

/** The branch slug a state file belongs to: `feat_x.waiver.json` and `feat_x.pending-waiver.json` are `feat_x`. */
export function slugOfStateFile(file: string): string {
  return basename(file)
    .replace(/\.json$/, "")
    .replace(/\.pending-waiver$/, "")
    .replace(/\.waiver$/, "");
}

function remove(file: string, warn: Warn): void {
  try {
    rmSync(file);
  } catch {
    warn(`toolu-sweep: could not remove ${file}`);
  }
}

type Branches = { current: string; live: Set<string>; merged: Set<string> };

function reclaimable(
  file: string,
  slug: string,
  branches: Branches,
  ttlHours: number,
  now: number,
): boolean {
  if (branches.merged.has(slug) || !branches.live.has(slug)) return true;
  // A live, unmerged branch: age is the only remaining reason (`find -mmin +TTL*60`).
  return now - statSync(file).mtimeMs > ttlHours * HOUR_MS;
}

function sweepBranchState(
  root: string,
  stateRoot: string,
  ttlHours: number,
  o: SweepContext,
): void {
  const branches: Branches = {
    current: branchSlug(currentBranch(root, o.env)),
    live: branchSlugs(root, o.env),
    // `--merged` sees ancestry only; squash-merged branches age out through the TTL.
    merged: branchSlugs(root, o.env, baseBranch(root, o.env)),
  };
  for (const dir of SWEEP_BRANCH_DIRS) {
    for (const file of globFiles(join(stateRoot, dir), ".json")) {
      const slug = slugOfStateFile(file);
      if (slug === branches.current) continue;
      if (reclaimable(file, slug, branches, ttlHours, o.now.getTime())) remove(file, o.warn);
    }
  }
}

/** Every path a failing gate record complains about; `__global__` is always live. */
function hasLiveViolation(keys: string[]): boolean {
  return keys.some((key) => key === GLOBAL_GATE_KEY || (key !== "" && existsSync(key)));
}

function sweepGateFile(gateFile: string, warn: Warn): void {
  withLock(
    gateFile,
    () => {
      const read = readGateFile(gateFile);
      if (read.kind !== "ok") return;
      const doc = read.doc;
      if (doc.status === "passing") {
        remove(gateFile, warn);
      } else if (
        !hasLiveViolation(doc.entries === undefined ? [doc.file] : Object.keys(doc.entries))
      ) {
        // Age alone never clears a failure: that would silently reopen a real violation.
        remove(gateFile, warn);
      }
    },
    { warn },
  );
}

/**
 * Lines of a telemetry file at or after `cutoff`, as `jq -c 'select((.t // "") >= $cutoff)'`
 * prints them; undefined when jq would fail (bad JSON, or a value that is neither an object nor null).
 */
export function keptTelemetryLines(content: string, cutoff: string): string[] | undefined {
  const kept: string[] = [];
  for (const line of content.split("\n")) {
    if (line.trim() === "") continue;
    let value: unknown;
    try {
      value = JSON.parse(line);
    } catch {
      return undefined;
    }
    if (value === null) continue;
    if (!isJsonObject(value)) return undefined;
    const t = value.t;
    // jq order: null/false/true/numbers < strings < arrays/objects.
    const keep =
      typeof t === "string"
        ? compareJqStrings(t, cutoff) >= 0
        : t !== null && typeof t === "object";
    if (keep) kept.push(toJqJson(value, false));
  }
  return kept;
}

function sweepTelemetry(dir: string, retentionDays: number, o: SweepContext): void {
  const cutoff = isoSeconds(new Date(o.now.getTime() - retentionDays * DAY_MS));
  for (const file of globFiles(dir, ".jsonl")) {
    const kept = keptTelemetryLines(readFileSync(file, "utf8"), cutoff);
    if (kept === undefined) continue;
    if (kept.length === 0) {
      remove(file, o.warn);
    } else if (!writeAtomic(file, `${kept.join("\n")}\n`)) {
      o.warn(`toolu-sweep: could not trim ${file}`);
    }
  }
}

type SweepContext = { env: NonNullable<StateOptions["env"]>; warn: Warn; now: Date };

function sweep(rootArg: string | undefined, options: StateOptions, warn: Warn): void {
  const env = options.env ?? process.env;
  const host = options.host ?? options.config?.host;
  const scoped = host === undefined ? { env } : { env, host };
  const root = rootArg === undefined || rootArg === "" ? projectRoot(scoped) : rootArg;
  if (root === undefined || root === "") return;
  const config = options.config ?? loadConfig({ ...scoped, cwd: root, warn });
  if (!enabled(config, "gates", "sweep") || !hasGit(env)) return;
  const stateRoot = projectStateRoot({ ...scoped, host: config.host, root });
  if (stateRoot === undefined || !existsSync(stateRoot) || !statSync(stateRoot).isDirectory())
    return;
  const context = { env, warn, now: options.now?.() ?? new Date() };
  sweepBranchState(
    root,
    stateRoot,
    positiveGate(config, "stateTtlHours", SWEEP_DEFAULT_TTL_HOURS),
    context,
  );
  sweepGateFile(join(stateRoot, "quality-gate-status.json"), warn);
  sweepTelemetry(
    join(stateRoot, "telemetry"),
    positiveGate(config, "telemetryRetentionDays", SWEEP_DEFAULT_RETENTION_DAYS),
    context,
  );
}

/** `toolu_sweep_state ROOT`: reclaim spent transient state. Never throws. */
export function sweepState(root?: string, options: StateOptions = {}): void {
  const warn = options.warn ?? stderrWarn;
  try {
    sweep(root, options, warn);
  } catch (error) {
    warn(`toolu-sweep: ${String(error)}`);
  }
}
