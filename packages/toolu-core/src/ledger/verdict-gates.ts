/**
 * The quality and plan push gates as `verdict.sh` reads them (#256), plus the
 * helpers every verdict gate shares. Each gate is a read-only mirror of its
 * real enforcement point. It never writes state or runs a check, and it
 * reports one gate object: `{state, reason, ...extra}`.
 */
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { isAbsolute, join, resolve } from "node:path";
import { childEnv, envValue, type HostEnv, type HostName } from "../host/host-name.ts";
import { branchSlug } from "../state/state-git.ts";
import { readLedger, stateDirFor, stateRootFor, type LedgerOptions } from "./ledger-io.ts";
import {
  JqError,
  alt,
  each,
  get,
  holds,
  jqEquals,
  length,
  parseJson,
  raw,
  type Json,
  type JsonObject,
} from "./ledger-jq.ts";
import { docField, isFile, isSpecless, parseAcs } from "./ledger-parse.ts";

export type GateState = "pass" | "fail" | "skip" | "advise" | "escalate";
export type Gate = JsonObject & { state: GateState; reason: string };

/** What every gate reads: the repo, the branch and base, the current diff hash and the environment. */
export type GateContext = {
  root: string;
  branch: string;
  base: string;
  cur: string;
  env: HostEnv;
  host?: HostName;
  cwd: string;
  /** Config warnings, as bash's loader prints them to stderr. */
  warn: (line: string) => void;
};

export function gate(state: GateState, reason: string, extra: JsonObject = {}): Gate {
  return { state, reason, ...extra };
}

/** `git -C ROOT ARGS...` stdout, or undefined on a non-zero exit. */
export function git(ctx: GateContext, args: string[]): string | undefined {
  const res = spawnSync("git", ["-C", ctx.root, ...args], {
    env: childEnv(ctx.env),
    encoding: "utf8",
  });
  return res.error === undefined && res.status === 0 ? res.stdout : undefined;
}

/** A JSON file as jq reads it, or undefined when absent or unparseable. */
export function readJson(file: string): Json | undefined {
  try {
    return parseJson(readFileSync(file, "utf8"));
  } catch {
    return undefined;
  }
}

/** `jq -r` of a computed value, where a jq error reads as `fallback`. */
export function rawOr(fn: () => Json, fallback = ""): string {
  try {
    return raw(fn());
  } catch (error) {
    if (error instanceof JqError) return fallback;
    throw error;
  }
}

export function hostOptions(ctx: GateContext): LedgerOptions {
  return ctx.host === undefined ? { env: ctx.env } : { env: ctx.env, host: ctx.host };
}

/** `${OVERRIDE:-$(toolu_project_state_dir NAME ROOT)}`. */
export function stateDir(ctx: GateContext, name: string, override: string): string {
  return envValue(ctx.env, override) ?? stateDirFor(name, ctx.root, hostOptions(ctx));
}

/** `vd_gate_quality`: a linked worktree skips; otherwise the gate file's top-level status decides. */
export function qualityGate(ctx: GateContext): Gate {
  // Same flags and `|| true` degradation as verdict.sh:75-76; `--path-format` needs Git 2.31+,
  // and on older Git both implementations read neither dir and skip the worktree check alike.
  const dir = (flag: string): string =>
    (git(ctx, ["rev-parse", "--path-format=absolute", flag]) ?? "").trim().replace(/\/$/, "");
  const gitDir = dir("--git-dir");
  const commonDir = dir("--git-common-dir");
  if (gitDir !== "" && commonDir !== "" && gitDir !== commonDir) {
    return gate("skip", "gate disabled in linked worktree");
  }
  const file = join(stateRootFor(ctx.root, hostOptions(ctx)), "quality-gate-status.json");
  if (!isFile(file)) return gate("pass", "no quality-gate failure recorded");
  const doc = readJson(file) ?? null;
  if (rawOr(() => alt(get(doc, "status"), "")) !== "failing")
    return gate("pass", "quality gate passing");
  const reason = rawOr(
    () => alt(get(doc, "reason"), "Quality gate failing"),
    "Quality gate failing",
  );
  return gate("fail", reason.replace(/\n+$/, ""));
}

const ZERO_PLAN: JsonObject = { summary: { total: 0, fresh_green: 0 }, ac_uncovered: 0 };
const CODE_FILE = /\.(ts|tsx|js|jsx|rs|sh|py|go)$/;

const freshAt = (step: Json, cur: string): boolean =>
  jqEquals(get(step, "status"), "green") && jqEquals(get(step, "diff_sha"), cur);

/** `id: effective-status` for each step that is not fresh-green; a jq error keeps the lines before it. */
function planBlockers(ledger: Json, cur: string): string[] {
  const lines: string[] = [];
  try {
    for (const step of each(get(ledger, "steps"))) {
      const green = jqEquals(get(step, "status"), "green");
      const eff: Json = freshAt(step, cur)
        ? "green"
        : green
          ? "stale"
          : alt(get(step, "status"), "pending");
      if (jqEquals(eff, "green")) continue;
      const id = alt(get(step, "id"), "?");
      if (typeof id !== "string" || typeof eff !== "string")
        throw new JqError("cannot add non-strings");
      lines.push(`${id}: ${eff}`);
    }
  } catch (error) {
    if (!(error instanceof JqError)) throw error;
  }
  return lines;
}

/** bash: an absolute path as is; a relative one from cwd when it exists there, else under the repo root. */
function near(ctx: GateContext, path: string): string {
  return isAbsolute(path) || isFile(resolve(ctx.cwd, path))
    ? resolve(ctx.cwd, path)
    : join(ctx.root, path);
}

/** The report-only count of spec ACs with no fresh-green covering step. */
function acUncovered(ctx: GateContext, ledger: Json): number {
  const planDoc = rawOr(() => alt(get(ledger, "plan_doc"), ""));
  if (planDoc === "" || !isFile(near(ctx, planDoc))) return 0;
  const specField = docField(near(ctx, planDoc), "Spec");
  if (isSpecless(specField) || !isFile(near(ctx, specField))) return 0;
  try {
    const steps = each(get(ledger, "steps"));
    return parseAcs(near(ctx, specField)).filter((ac) => {
      // jq builds the whole covering list first, so a bad ac_refs anywhere fails the count.
      const covering = steps.filter((step) => holds(alt(get(step, "ac_refs"), []), ac));
      return !covering.some((step) => freshAt(step, ctx.cur));
    }).length;
  } catch (error) {
    if (!(error instanceof JqError)) throw error;
    return 0;
  }
}

/** `vd_gate_plan`: the plan-ledger push check, recomputed against the current diff. */
export function planGate(ctx: GateContext): Gate {
  const file = join(stateDir(ctx, "plan-ledger", "LEDGER_DIR"), `${branchSlug(ctx.branch)}.json`);
  if (!isFile(file)) {
    // Parity with verdict.sh:113 (`2>/dev/null ... || true`): a failed diff reads as no code
    // files. The failure is not hidden: an unresolvable base also empties the diff hash, and
    // the review gate then reports "could not compute diff against <base>".
    const names = git(ctx, ["diff", "--no-color", `${ctx.base}...HEAD`, "--name-only"]) ?? "";
    return names.split("\n").some((name) => CODE_FILE.test(name))
      ? gate("advise", "no plan ledger for this change; if non-trivial, run plan", ZERO_PLAN)
      : gate("skip", "no plan ledger and no code files in diff", ZERO_PLAN);
  }
  const read = readLedger(file);
  if (read === undefined) return gate("fail", `unparseable ledger at ${file}`, ZERO_PLAN);
  const ledger = read.value;
  const version = rawOr(() => alt(get(ledger, "version"), ""));
  if (version !== "1") {
    return gate(
      "fail",
      `ledger schema mismatch at ${file} (version="${version}", expected 1)`,
      ZERO_PLAN,
    );
  }
  const total = rawOr(() => alt(get(get(ledger, "summary"), "total"), 0), "0");
  const count = rawOr(() => length(get(ledger, "steps")), "0");
  if (total === "0" || count === "0") return gate("skip", "empty plan ledger", ZERO_PLAN);
  let extra: JsonObject = {};
  try {
    const fresh = each(get(ledger, "steps")).filter((step) => freshAt(step, ctx.cur)).length;
    const summary = { total: length(get(ledger, "steps")), fresh_green: fresh };
    extra = { summary, ac_uncovered: acUncovered(ctx, ledger) };
  } catch (error) {
    // bash: the summary jq fails, then `--argjson summary ""` fails, and vd_gate falls back to `{}`.
    if (!(error instanceof JqError)) throw error;
  }
  const blockers = planBlockers(ledger, ctx.cur);
  if (blockers.length === 0) return gate("pass", "all plan-ledger steps fresh-green", extra);
  return gate("fail", `steps not fresh-green: ${blockers.join(",")}`, extra);
}
