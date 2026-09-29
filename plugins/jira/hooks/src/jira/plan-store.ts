/**
 * The plan ledger store: paths, the document build/merge, summary recompute,
 * and an atomic write. It emits schema version 1, the document toolu's
 * dashboard (plugins/toolu/dashboard/state.ts) reads. Two fields are
 * load-bearing:
 *   base_branch: "" — the dashboard's currentDiffSha() returns null for an
 *                     empty base, so no Jira step goes stale on code commits.
 *   diff_sha: null  — a Jira step's freshness never depends on the code diff.
 * The file is jira-<KEY>.json, never <branch-slug>.json, so toolu's push gate
 * (which stats only <branch-slug>.json) can never block a push on Jira state.
 */
import { CliExit } from "@toolu/core/cli";
import { formatJson } from "@toolu/core/rest";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import type { Env } from "./creds.ts";
import { alt, isObject, type JsonObject } from "./jq.ts";
import { KEY_PATTERN, type Step } from "./plan-parse.ts";

export interface Summary {
  readonly total: number;
  readonly green: number;
  readonly red: number;
  readonly pending: number;
  readonly running: number;
  readonly stale: 0;
  readonly fresh_green: number;
  readonly retried: 0;
}

export interface Ledger {
  readonly version: 1;
  readonly branch: string;
  readonly base_branch: "";
  readonly plan_doc: string;
  readonly updated_at: string;
  readonly summary: Summary;
  readonly next: unknown;
  readonly steps: readonly JsonObject[];
}

/** The repository root of `cwd`, or `cwd` itself outside a git work tree. */
export function repoRoot(cwd: string): string {
  const run = spawnSync("git", ["rev-parse", "--show-toplevel"], { cwd, encoding: "utf8" });
  return run.status === 0 ? run.stdout.replace(/\n+$/, "") : cwd;
}

/** Codex when forced, or when only Codex's PLUGIN_ROOT lifecycle variable is present. */
export function isCodex(env: Env): boolean {
  const host = env["TOOLU_HOST_OVERRIDE"] ?? "";
  return host === "codex" || (host === "" && Boolean(env["PLUGIN_ROOT"]));
}

/** The host-native project state directory: `.claude`, or `.codex` on Codex. */
function projectDir(env: Env): string {
  return env["TOOLU_PROJECT_CONFIG_DIRNAME"] || (isCodex(env) ? ".codex" : ".claude");
}

/** Exit 1 unless `key` is a Jira key: it is interpolated into file names. */
export function requireKey(key: string): void {
  if (!KEY_PATTERN.test(key)) throw new CliExit(1, `jira plan: not a valid issue key: '${key}'`);
}

/** `<repo>/<host-dir>/tmp/plan-ledger/jira-<KEY>.json`. */
export function ledgerPath(key: string, env: Env, cwd: string): string {
  requireKey(key);
  return join(repoRoot(cwd), projectDir(env), "tmp", "plan-ledger", `jira-${key}.json`);
}

/** `<repo>/<host-dir>/tmp/jira/plans/<KEY>.md`. */
export function docPath(key: string, env: Env, cwd: string): string {
  requireKey(key);
  return join(repoRoot(cwd), projectDir(env), "tmp", "jira", "plans", `${key}.md`);
}

/** ISO-8601 UTC, second precision. */
export function now(): string {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}

/** The last 10 lines of a check's output, capped to 2000 bytes (a cut character becomes U+FFFD). */
export function evidence(output: string): string {
  const tail = output.split("\n").slice(-10).join("\n");
  return new TextDecoder().decode(Buffer.from(tail).subarray(0, 2000));
}

/** The ledger object, or undefined when absent, empty, unparseable, or not an object. */
export function readLedger(file: string): JsonObject | undefined {
  let content: string;
  try {
    content = readFileSync(file, "utf8");
  } catch {
    return undefined;
  }
  try {
    const value: unknown = JSON.parse(content);
    return isObject(value) ? value : undefined;
  } catch {
    return undefined;
  }
}

/** Writes the ledger `jq .`-formatted through a temp file and a rename, creating the directory. */
export function writeLedger(file: string, ledger: Ledger): void {
  const dir = dirname(file);
  try {
    mkdirSync(dir, { recursive: true });
  } catch {
    throw new CliExit(1, `jira plan: cannot create ledger dir: ${dir}`);
  }
  const temp = `${file}.tmp.${process.pid}`;
  try {
    writeFileSync(temp, formatJson(ledger));
  } catch {
    rmSync(temp, { force: true });
    throw new CliExit(1, `jira plan: failed to stage ledger to ${temp}`);
  }
  try {
    renameSync(temp, file);
  } catch {
    rmSync(temp, { force: true });
    throw new CliExit(1, `jira plan: atomic mv failed for ${file}`);
  }
}

function count(steps: readonly JsonObject[], status: string): number {
  return steps.filter((step) => step["status"] === status).length;
}

/** The ledger with summary and next recomputed from its steps. */
export function recompute(ledger: Ledger): Ledger {
  const { steps } = ledger;
  const green = count(steps, "green");
  const summary: Summary = {
    total: steps.length,
    green,
    red: count(steps, "red"),
    pending: count(steps, "pending"),
    running: count(steps, "running"),
    stale: 0,
    fresh_green: green,
    retried: 0,
  };
  const next = alt(steps.find((step) => step["status"] !== "green")?.["id"], null);
  return { ...ledger, summary, next };
}

/** Previous steps by id; later duplicates win, as jq's `add` merged them. */
function previousSteps(prev: JsonObject | undefined): Map<string, JsonObject> {
  const steps = prev?.["steps"];
  const byId = new Map<string, JsonObject>();
  if (!Array.isArray(steps)) return byId;
  for (const step of steps) {
    if (isObject(step) && typeof step["id"] === "string") byId.set(step["id"], step);
  }
  return byId;
}

/**
 * The full ledger: authored fields (title/check/activity) from the doc,
 * execution state carried over from `prev` by step id, pending by default.
 * A step dropped from the doc disappears; a new one arrives pending.
 */
export function buildLedger(
  key: string,
  doc: string,
  steps: readonly Step[],
  prev: JsonObject | undefined,
): Ledger {
  const old = previousSteps(prev);
  return recompute({
    version: 1,
    branch: `jira-${key}`,
    base_branch: "",
    plan_doc: doc,
    updated_at: now(),
    summary: {
      total: 0,
      green: 0,
      red: 0,
      pending: 0,
      running: 0,
      stale: 0,
      fresh_green: 0,
      retried: 0,
    },
    next: null,
    steps: steps.map((step) => {
      const p = old.get(step.id) ?? {};
      return {
        id: step.id,
        title: step.title,
        check: step.check,
        status: alt(p["status"], "pending"),
        started_at: alt(p["started_at"], null),
        activity: alt(step.activity, alt(p["activity"], null)),
        exit_code: alt(p["exit_code"], null),
        diff_sha: null,
        last_run: alt(p["last_run"], null),
        evidence_tail: alt(p["evidence_tail"], null),
      };
    }),
  });
}

/** Merges `fields` into step `id`, restamps updated_at, and recomputes. */
export function setStep(ledger: Ledger, id: string, fields: JsonObject): Ledger {
  return recompute({
    ...ledger,
    updated_at: now(),
    steps: ledger.steps.map((step) => (step["id"] === id ? { ...step, ...fields } : step)),
  });
}
