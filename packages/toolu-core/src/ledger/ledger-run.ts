/**
 * `plan-ledger.sh run` (#256), a port of `pl_cmd_run`. It parses the plan,
 * runs each step's check, stamps the mechanical status and content-addressed
 * hashes, and writes the per-branch ledger. The exit code is 0 when every step
 * is fresh-green, 1 otherwise, and 2 on a parse or I/O error, in which case
 * nothing is written. The agent never claims green; the check's exit code
 * decides.
 */
import { mkdirSync, readFileSync, rmSync } from "node:fs";
import { dirname } from "node:path";
import { childEnv } from "../host/host-name.ts";
import { branchSlug } from "../state/state-git.ts";
import { telemetryAppend } from "../state/telemetry.ts";
import { runCheck, stepEvidence } from "./ledger-check.ts";
import { Output, type CommandResult, type LedgerOptions, type ReadLedger } from "./ledger-io.ts";
import {
  JqError,
  alt,
  get,
  isObject,
  jqEquals,
  raw,
  type Json,
  type JsonObject,
} from "./ledger-jq.ts";
import {
  allFresh,
  buildStepEntry,
  carryForward,
  pendingEntry,
  recompute,
  refreshAuthored,
  runningEntry,
  stepsWithId,
  summaryLine,
} from "./ledger-model.ts";
import {
  CommandFail,
  ledgerDoc,
  orFail,
  parseRunFlags,
  prepare,
  writeOrFail,
  type RunContext,
} from "./ledger-run-context.ts";
import { scopeMap } from "./ledger-scope.ts";

/** `--step` write #1: the target step marked `running` before its check starts. */
function preWrite(ctx: RunContext): void {
  const startedAt = ctx.now();
  const steps = orFail(
    () =>
      ctx.steps.flatMap((step) => {
        const prior = ctx.existing.get(step.id) ?? null;
        return stepsWithId(ctx.steps, step.id).map((match) => {
          if (step.id === ctx.onlyStep) return runningEntry(match, prior, startedAt, ctx.activity);
          return prior === null ? pendingEntry(match) : refreshAuthored(prior, match);
        });
      }),
    "plan-ledger: failed to assemble running pre-write",
  );
  // bash reads `$scope_map` here before assigning it: the pre-write is judged without scopes.
  const ledger = orFail(
    () => recompute(ledgerDoc(ctx, startedAt, steps), ctx.cur, {}, ctx.verify),
    "plan-ledger: failed to recompute running pre-ledger",
  );
  writeOrFail(ctx, ledger, "plan-ledger: running pre-write failed");
}

function progress(ctx: RunContext, message: string): void {
  ctx.out.stderr(`plan-ledger: ${message}`);
}

/** Run one step's check and build its finished entry (one per duplicate of `id`). */
async function runStep(
  ctx: RunContext,
  id: string,
  matches: Json[],
  prior: Json,
  at: string,
  scopeNow: string,
) {
  const tmp = `${ctx.ledgerFile}.run.${process.pid}.${id}`;
  try {
    mkdirSync(dirname(ctx.ledgerFile), { recursive: true });
  } catch {
    // bash: `mkdir -p ... || true`; the check's output redirect reports the real problem.
  }
  progress(ctx, `${at} ${id}: running check`);
  const check = matches
    .map((m) => raw(get(m, "check")))
    .join("\n")
    .replace(/\n+$/, "");
  const t0 = Math.floor(Date.now() / 1000);
  let code = 1;
  let output: Uint8Array = new Uint8Array();
  try {
    code = await runCheck({
      check,
      cwd: ctx.root,
      env: childEnv(ctx.env),
      outFile: tmp,
      timeout: ctx.timeout,
    });
    output = readFileSync(tmp);
  } catch {
    // bash: a failed `> "$outfile"` redirect leaves exit 1 and no output to read.
  } finally {
    rmSync(tmp, { force: true });
  }
  const duration = Math.floor(Date.now() / 1000) - t0;
  const status = code === 0 ? "green" : "red";
  const evidence = stepEvidence(code, output, ctx.timeout);
  progress(ctx, `${at} ${id}: ${status} (${String(duration)}s)`);
  const run = { status, exitCode: code, sha: ctx.cur, evidence, now: ctx.now() } as const;
  const entries: JsonObject[] = orFail(
    () =>
      matches.map((m): JsonObject => ({
        ...buildStepEntry(m, prior, run),
        scope_sha: scopeNow === "" ? null : scopeNow,
      })),
    `plan-ledger: failed to build entry for step ${id}`,
  );
  const [entry] = entries;
  // A duplicated id yields no single entry: bash's telemetry extras are then malformed (D5).
  if (entries.length === 1 && entry !== undefined) {
    const retries = entry.retries;
    const attempt = (Array.isArray(retries) ? retries.length : 0) + 1;
    const extras = { step_id: id, status, exit_code: code, duration_s: duration, attempt };
    const warn = (line: string): void => ctx.out.stderr(line);
    const hostOpt = ctx.options.host === undefined ? {} : { host: ctx.options.host };
    const nowOpt = ctx.options.now === undefined ? {} : { now: ctx.options.now };
    telemetryAppend(ctx.root, "step_run", extras, { env: ctx.env, warn, ...hostOpt, ...nowOpt });
  }
  return entries;
}

/** A step already fresh-green at its key is carried forward; anything else runs. */
async function runOrSkip(
  ctx: RunContext,
  id: string,
  matches: Json[],
  at: string,
): Promise<Json[]> {
  const prior = ctx.existing.get(id) ?? null;
  const scoped = ctx.scope[id];
  const scopeNow = typeof scoped === "string" ? scoped : "";
  const useScope = !ctx.verify && scopeNow !== "";
  const priorKey = raw(alt(get(prior, useScope ? "scope_sha" : "diff_sha"), ""));
  const nowKey = useScope ? scopeNow : ctx.cur;
  const green = raw(alt(get(prior, "status"), "")) === "green";
  if (!ctx.force && ctx.onlyStep === "" && green && nowKey !== "" && priorKey === nowKey) {
    progress(ctx, `${at} ${id}: fresh-green, skipped (--force re-runs)`);
    return orFail(
      () => matches.map((m) => carryForward(prior, m)),
      `plan-ledger: failed to carry forward step ${id}`,
    );
  }
  return runStep(ctx, id, matches, prior, at, scopeNow);
}

async function runSteps(ctx: RunContext): Promise<Json[]> {
  const out: Json[] = [];
  let index = 0;
  // Strictly one after another, in plan order, as bash runs them.
  await ctx.steps.reduce<Promise<void>>(async (previous, { id }) => {
    await previous;
    const matches = stepsWithId(ctx.steps, id);
    let entries: Json[];
    if (ctx.onlyStep !== "" && id !== ctx.onlyStep) {
      const prior = ctx.existing.get(id) ?? null;
      entries = orFail(
        () => matches.map((m) => (prior === null ? pendingEntry(m) : carryForward(prior, m))),
        `plan-ledger: failed to assemble entry for step ${id}`,
      );
    } else {
      index += 1;
      entries = await runOrSkip(ctx, id, matches, `[${String(index)}/${String(ctx.steps.length)}]`);
    }
    const [entry] = entries;
    if (entries.length !== 1 || entry === undefined) {
      throw new CommandFail([`plan-ledger: failed to append step ${id}`]);
    }
    out.push(entry);
  }, Promise.resolve());
  return out;
}

/** The prior ledger's `verified_sha` as `jq -r '.verified_sha // ""'` reads it. */
function priorVerified(prior: ReadLedger | undefined): string {
  try {
    return prior === undefined ? "" : raw(alt(get(prior.value, "verified_sha"), ""));
  } catch (error) {
    if (error instanceof JqError) return "";
    throw error;
  }
}

/** Assemble, recompute, stamp `verified_sha`, write, and print the summary line. */
function finish(ctx: RunContext, steps: Json[]): Json {
  const ledger = orFail(
    () => recompute(ledgerDoc(ctx, ctx.now(), steps), ctx.cur, ctx.scope, ctx.verify),
    "plan-ledger: failed to recompute summary",
  );
  if (!isObject(ledger)) throw new CommandFail(["plan-ledger: failed to recompute summary"]);
  const summary = get(ledger, "summary");
  const verified =
    ctx.verify &&
    ctx.onlyStep === "" &&
    jqEquals(get(summary, "fresh_green"), get(summary, "total"));
  const previous = priorVerified(ctx.prior);
  const stamped = {
    ...ledger,
    verified_sha: verified ? ctx.cur : previous === "" ? null : previous,
  };
  writeOrFail(ctx, stamped, "plan-ledger: ledger write failed");
  try {
    ctx.out.stdout(`${summaryLine(stamped, branchSlug(ctx.branch))}\n`);
  } catch (error) {
    // bash prints jq's error and still exits on freshness alone.
    if (!(error instanceof JqError)) throw error;
  }
  return stamped;
}

/** `plan-ledger.sh run DOC [--step ID] [--activity LABEL] [--force] [--verify]`. */
export async function ledgerRun(
  doc: string,
  flags: readonly string[],
  options: LedgerOptions = {},
): Promise<CommandResult> {
  const out = new Output(options.onStderr);
  try {
    const ctx = prepare(doc, parseRunFlags(flags), options, out);
    if (ctx.onlyStep !== "") preWrite(ctx);
    ctx.scope = scopeMap(ctx.steps, ctx.base, ctx.root, ctx.env, (line) => out.stderr(line));
    const ledger = finish(ctx, await runSteps(ctx));
    return out.result(allFresh(ledger) ? 0 : 1);
  } catch (error) {
    if (!(error instanceof CommandFail)) throw error;
    for (const line of error.lines) out.stderr(line);
    return out.result(2);
  }
}
