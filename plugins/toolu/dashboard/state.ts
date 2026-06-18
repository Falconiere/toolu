// Pure, cwd-independent dashboard state derivation over the plan-ledger JSON.
// Reads the ledger file, computes the repo's current diff_sha via git, and
// derives per-step `stale`/`stuck`. Never throws: every fs/git/JSON failure is
// caught and turned into a well-shaped payload so the server never 500s.

import { spawnSync } from "node:child_process";
import { readFileSync, statSync } from "node:fs";

/** A single ledger step as persisted by plan-ledger.sh (schema version 1). */
export interface LedgerStep {
  id: string;
  title: string;
  check: string;
  status: "green" | "red" | "pending" | "running";
  started_at: string | null;
  activity: string | null;
  exit_code: number | null;
  diff_sha: string | null;
  last_run: string | null;
  evidence_tail: string | null;
}

/** The full per-branch ledger document. */
export interface Ledger {
  version: number;
  branch: string;
  base_branch: string;
  plan_doc: string;
  updated_at: string;
  summary: Record<string, number>;
  next: string | null;
  steps: LedgerStep[];
}

/** A step enriched with the two derived flags the kanban renders. */
export interface DerivedStep extends LedgerStep {
  stale: boolean;
  stuck: boolean;
}

/** The payload served at /api/state and pushed over SSE. */
export interface DashboardState {
  ledger: Ledger | null;
  currentDiffSha: string | null;
  steps: DerivedStep[];
  serverTime: string;
  /** Set only when the ledger file exists but is unreadable/corrupt. */
  error?: string;
}

/** Inputs are injected explicitly so the module is pure and testable. */
export interface BuildStateOptions {
  ledgerPath: string;
  repoRoot: string;
  /** running-age (seconds) past which a step is `stuck`. Default 300. */
  stuckThresholdSeconds?: number;
}

const DEFAULT_STUCK_SECONDS = 300;

/** Compute `git diff --no-color <base>...HEAD | git hash-object --stdin` against
 *  repoRoot (matching pl_diff_sha). Returns null on any git failure or empty sha. */
export function currentDiffSha(repoRoot: string, base: string): string | null {
  if (!base) return null;
  const diff = spawnSync("git", ["-C", repoRoot, "diff", "--no-color", `${base}...HEAD`], {
    encoding: "buffer",
    maxBuffer: 256 * 1024 * 1024,
  });
  if (diff.status !== 0 || diff.error) return null;
  const hash = spawnSync("git", ["-C", repoRoot, "hash-object", "--stdin"], {
    input: diff.stdout,
    encoding: "utf8",
  });
  if (hash.status !== 0 || hash.error) return null;
  const sha = hash.stdout.trim();
  return sha.length > 0 ? sha : null;
}

/** True when a running step's started_at is older than the stuck threshold. */
function isStuck(step: LedgerStep, now: number, thresholdSeconds: number): boolean {
  if (step.status !== "running" || !step.started_at) return false;
  const startedMs = Date.parse(step.started_at);
  if (Number.isNaN(startedMs)) return false;
  return (now - startedMs) / 1000 > thresholdSeconds;
}

/** Read the ledger, compute current diff_sha, and derive stale/stuck per step.
 *  Absent ledger -> {ledger:null,currentDiffSha:null,steps:[]}. Corrupt JSON ->
 *  error-shaped payload. Git failure -> currentDiffSha:null and all stale:false. */
export function buildState(opts: BuildStateOptions): DashboardState {
  const { ledgerPath, repoRoot } = opts;
  const threshold = opts.stuckThresholdSeconds ?? DEFAULT_STUCK_SECONDS;
  const serverTime = new Date().toISOString();

  let raw: string;
  try {
    statSync(ledgerPath);
    raw = readFileSync(ledgerPath, "utf8");
  } catch {
    return { ledger: null, currentDiffSha: null, steps: [], serverTime };
  }
  if (raw.trim().length === 0) {
    return { ledger: null, currentDiffSha: null, steps: [], serverTime };
  }

  let ledger: Ledger;
  try {
    ledger = JSON.parse(raw) as Ledger;
  } catch {
    return {
      ledger: null,
      currentDiffSha: null,
      steps: [],
      serverTime,
      error: "ledger json is corrupt",
    };
  }

  const sha = currentDiffSha(repoRoot, ledger.base_branch);
  const now = Date.now();
  const steps: DerivedStep[] = (ledger.steps ?? []).map((step) => ({
    ...step,
    // stale only when we have a real current sha to compare against; on git
    // failure (sha === null) every step reads stale:false, never all-stale.
    stale: sha !== null && step.status === "green" && step.diff_sha !== sha,
    stuck: isStuck(step, now, threshold),
  }));

  return { ledger, currentDiffSha: sha, steps, serverTime };
}
