/**
 * The pure half of `plan-ledger.sh` (#256): summary recompute, the summary
 * line, step-entry construction and carry-forward, orphan healing, and the
 * AC-coverage report. Each function is a direct port of its jq program, so it
 * throws `JqError` wherever jq fails on a malformed ledger.
 */
import { compareJqStrings, isoSeconds, toJqJson } from "../state/state-io.ts";
import {
  JqError,
  alt,
  concat,
  each,
  get,
  holds,
  isObject,
  jqEquals,
  jqType,
  length,
  toStr,
  type Json,
  type JsonObject,
} from "./ledger-jq.ts";
import { isSpecless, parseAcs } from "./ledger-parse.ts";

/** A step object in the ledger file, as `pl_build_step_entry` writes it. */
export type LedgerStep = {
  id: string;
  title: string;
  check: string;
  status: "pending" | "running" | "green" | "red";
  started_at: string | null;
  activity: string | null;
  exit_code: number | null;
  diff_sha: string | null;
  last_run: string | null;
  evidence_tail: string | null;
  ac_refs: Json;
  depends_on: Json;
  input: Json;
  model: Json;
  retries: Json[];
  scope_sha?: string | null;
};

/** The ledger file (`version: 1`). */
export type LedgerDoc = {
  version: 1;
  branch: string;
  base_branch: string;
  plan_doc: string;
  updated_at: string;
  summary: Record<string, number>;
  next: string | null;
  steps: LedgerStep[];
  verified_sha?: string | null;
};

const statusIs = (step: Json, status: string): boolean => jqEquals(get(step, "status"), status);

/** `is_fresh` from `pl_recompute`, with jq's short-circuiting `and`. */
function isFresh(step: Json, cur: string, scope: JsonObject, verify: boolean): boolean {
  if (!statusIs(step, "green")) return false;
  if (!verify) {
    const scoped = alt(get(scope, get(step, "id")), null);
    if (scoped !== null) {
      const own = get(step, "scope_sha");
      return own !== null && jqEquals(own, scoped);
    }
  }
  return jqEquals(get(step, "diff_sha"), cur);
}

/** jq `. + {key: value}` on an object: an existing key keeps its place. */
function assign(target: Json, key: string, value: Json): JsonObject {
  if (target !== null && !isObject(target)) {
    throw new JqError(`Cannot index ${jqType(target)} with "${key}"`);
  }
  return { ...target, [key]: value };
}

/**
 * `pl_recompute LEDGER CUR [SCOPE] [VERIFY]`: summary counts and `next`
 * against the current branch hash. A step with a declared scope is judged on
 * its scope hash unless `verify`.
 */
export function recompute(ledger: Json, cur: string, scope: JsonObject = {}, verify = false): Json {
  const steps = each(get(ledger, "steps"));
  const count = (pred: (step: Json) => boolean): number => steps.filter(pred).length;
  const fresh = (step: Json): boolean => isFresh(step, cur, scope, verify);
  const summary: JsonObject = {
    total: length(get(ledger, "steps")),
    green: count((s) => statusIs(s, "green")),
    red: count((s) => statusIs(s, "red")),
    pending: count((s) => statusIs(s, "pending")),
    running: count((s) => statusIs(s, "running")),
    stale: count((s) => statusIs(s, "green") && !fresh(s)),
    fresh_green: count(fresh),
    retried: count((s) => length(alt(get(s, "retries"), [])) > 0),
  };
  const withSummary = assign(ledger, "summary", summary);
  const firstStale = steps.find((s) => !fresh(s));
  const next = firstStale === undefined ? null : alt(get(firstStale, "id"), null);
  return assign(withSummary, "next", next);
}

/** `pl_summary_line`: `plan-ledger <slug>: <fresh>/<total> fresh-green, next=<id|none>[ model=<m>]`. */
export function summaryLine(ledger: Json, slug: string): string {
  const next = get(ledger, "next");
  const model =
    each(get(ledger, "steps"))
      .filter((step) => jqEquals(get(step, "id"), next))
      .map((step) => get(step, "model"))
      .find((m) => m !== null) ?? null;
  const summary = get(ledger, "summary");
  return concat(
    "plan-ledger ",
    slug,
    ": ",
    toStr(get(summary, "fresh_green")),
    "/",
    toStr(get(summary, "total")),
    " fresh-green, next=",
    alt(next, "none"),
    model === null ? "" : concat(" model=", model),
  );
}

/** `pl_all_fresh`: every step fresh-green, i.e. no `next`. */
export function allFresh(ledger: Json): boolean {
  return get(ledger, "next") === null;
}

/** `$steps[] | select(.id == $id)`: every matching step (duplicates included). */
export function stepsWithId(steps: readonly Json[], id: Json): Json[] {
  return steps.filter((step) => jqEquals(get(step, "id"), id));
}

/** The authored fields a ledger entry re-derives from the plan on every run. */
function authored(step: Json): JsonObject {
  return {
    ac_refs: alt(get(step, "ac_refs"), []),
    depends_on: alt(get(step, "depends_on"), []),
    input: alt(get(step, "input"), null),
    model: alt(get(step, "model"), null),
  };
}

/** A never-run entry: the `pending` seed. */
export function pendingEntry(step: Json): JsonObject {
  return {
    id: get(step, "id"),
    title: get(step, "title"),
    check: get(step, "check"),
    status: "pending",
    started_at: null,
    activity: null,
    exit_code: null,
    diff_sha: null,
    last_run: null,
    evidence_tail: null,
    ...authored(step),
    retries: [],
  };
}

/** The `--step` pre-write entry: `running` since `now`. */
export function runningEntry(step: Json, prior: Json, now: string, activity: string): JsonObject {
  return {
    ...pendingEntry(step),
    status: "running",
    started_at: now,
    activity: activity === "" ? null : activity,
    last_run: now,
    retries: alt(get(prior, "retries"), []),
  };
}

/** Prior entry with the plan's authored fields re-applied (`--step` pre-write, non-target). */
export function refreshAuthored(prior: Json, step: Json): Json {
  let out = prior;
  for (const [key, value] of Object.entries(authored(step))) out = assign(out, key, value);
  return out;
}

/** Prior entry carried forward, every engine field backfilled (non-target step, or skipped fresh step). */
export function carryForward(prior: Json, step: Json): JsonObject {
  let out = assign(prior, "scope_sha", alt(get(prior, "scope_sha"), null));
  for (const [key, value] of Object.entries(authored(step))) out = assign(out, key, value);
  out = assign(out, "title", get(step, "title"));
  out = assign(out, "check", get(step, "check"));
  out = assign(out, "status", alt(get(out, "status"), "pending"));
  for (const key of [
    "started_at",
    "activity",
    "exit_code",
    "diff_sha",
    "last_run",
    "evidence_tail",
  ]) {
    out = assign(out, key, alt(get(out, key), null));
  }
  return assign(out, "retries", alt(get(out, "retries"), []));
}

/** jq `a + b` for the retries list: arrays concatenate, a null left side is the right side. */
function appendRetry(retries: Json, record: JsonObject): Json[] {
  if (retries === null) return [record];
  if (!Array.isArray(retries)) {
    throw new JqError(`${jqType(retries)} and array cannot be added`);
  }
  return [...retries, record];
}

export type RunOutcome = {
  status: "green" | "red";
  exitCode: number;
  sha: string;
  evidence: string;
  now: string;
};

/**
 * `pl_build_step_entry`: a finished step. A prior red entry is archived into
 * `retries`, and a prior's own retries carry forward.
 */
export function buildStepEntry(step: Json, prior: Json, run: RunOutcome): JsonObject {
  const priorRetries = alt(get(prior, "retries"), []);
  const retries = jqEquals(alt(get(prior, "status"), null), "red")
    ? appendRetry(priorRetries, {
        attempt: length(priorRetries) + 1,
        exit_code: get(prior, "exit_code"),
        diff_sha: get(prior, "diff_sha"),
        evidence_tail: get(prior, "evidence_tail"),
        at: get(prior, "last_run"),
      })
    : priorRetries;
  return {
    id: get(step, "id"),
    title: get(step, "title"),
    check: get(step, "check"),
    status: run.status,
    started_at: null,
    activity: null,
    exit_code: run.exitCode,
    diff_sha: run.sha,
    last_run: run.now,
    evidence_tail: run.evidence,
    ...authored(step),
    retries,
  };
}

/** jq's order between the orphan cutoff (a string) and a `started_at` of any type. */
function beforeCutoff(startedAt: Json, cutoff: string): boolean {
  if (typeof startedAt === "string") return compareJqStrings(startedAt, cutoff) < 0;
  return startedAt === null || typeof startedAt === "boolean" || typeof startedAt === "number";
}

/** `now - thresholdSeconds` in the ledger's ISO format. */
export function orphanCutoff(now: Date, thresholdSeconds: number): string {
  return isoSeconds(new Date(Math.floor(now.getTime() / 1000 - thresholdSeconds) * 1000));
}

/**
 * `pl_heal_orphans`: a `running` step whose `started_at` is missing, empty or
 * older than the cutoff goes back to `pending`.
 */
export function healOrphans(ledger: Json, cutoff: string): Json {
  const healed = each(get(ledger, "steps")).map((step) => {
    if (!statusIs(step, "running")) return step;
    const startedAt = alt(get(step, "started_at"), "");
    if (startedAt !== "" && !beforeCutoff(startedAt, cutoff)) return step;
    return assign(assign(assign(step, "status", "pending"), "started_at", null), "activity", null);
  });
  return assign(ledger, "steps", healed);
}

/** jq `join(", ")` over step ids: null joins as empty, scalars as their text. */
function joinIds(ids: Json[]): string {
  return ids
    .map((id) => {
      if (id === null) return "";
      if (typeof id === "object") throw new JqError(`Cannot join with ${jqType(id)}`);
      return typeof id === "string" ? id : toJqJson(id, false);
    })
    .join(", ");
}

/** One `pl_ac_coverage_lines` line for `ac`. */
export function acCoverageLine(ledger: Json, cur: string, ac: string): string {
  const covering = each(get(ledger, "steps")).filter((step) =>
    holds(alt(get(step, "ac_refs"), []), ac),
  );
  const ids = joinIds(covering.map((step) => get(step, "id")));
  const fresh = covering.some(
    (step) => statusIs(step, "green") && jqEquals(get(step, "diff_sha"), cur),
  );
  if (covering.length === 0) return `  ${ac}: UNCOVERED (no step references it)`;
  return fresh ? `  ${ac}: covered by ${ids}` : `  ${ac}: UNCOVERED (${ids} not fresh-green)`;
}

/**
 * `pl_ac_coverage_lines LEDGER CUR SPEC`: report-only. Spec-less or AC-less
 * specs print nothing, and a ledger that jq cannot read stops the report with
 * one tagged line; it never fails the caller.
 */
export function acCoverage(
  ledger: Json,
  cur: string,
  spec: string,
): { stdout: string; stderr: string[] } {
  if (isSpecless(spec)) return { stdout: "", stderr: [] };
  const acs = parseAcs(spec);
  if (acs.length === 0) return { stdout: "", stderr: [] };
  let stdout = "AC coverage (report-only):\n";
  for (const ac of acs) {
    try {
      stdout += `${acCoverageLine(ledger, cur, ac)}\n`;
    } catch (error) {
      if (!(error instanceof JqError)) throw error;
      return { stdout, stderr: [`plan-ledger: failed to compute AC coverage for ${ac}`] };
    }
  }
  return { stdout, stderr: [] };
}

/** jq `from_entries` over `{key: .id, value: .}`: the prior entries indexed by id. */
export function entriesById(ledger: Json): Map<string, Json> {
  const out = new Map<string, Json>();
  for (const step of each(get(ledger, "steps"))) {
    const id = get(step, "id");
    if (typeof id !== "string") throw new JqError(`Cannot use ${jqType(id)} as object key`);
    out.set(id, step);
  }
  return out;
}
