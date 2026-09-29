/**
 * The setup half of `pl_cmd_run` (#256): argument parsing, the resolved run
 * context (repo, base, ledger path, branch hash, prior entries), and the
 * failure type that ends a command with exit 2 after its tagged lines.
 */
import { statSync } from "node:fs";
import { envValue, type HostEnv } from "../host/host-name.ts";
import { diffSha } from "../state/diff-sha.ts";
import { baseBranch } from "../state/state-git.ts";
import { isoSeconds } from "../state/state-io.ts";
import {
  headBranch,
  ledgerPath,
  projectRoot,
  readLedger,
  writeLedger,
  type LedgerOptions,
  type Output,
  type ReadLedger,
} from "./ledger-io.ts";
import { JqError, type Json, type JsonObject } from "./ledger-jq.ts";
import { entriesById } from "./ledger-model.ts";
import { parseSteps, type PlanStep } from "./ledger-parse.ts";

/** A failure that ends the command with exit 2 after printing `lines`. */
export class CommandFail extends Error {
  constructor(readonly lines: string[]) {
    super(lines.join("\n"));
  }
}

/** Run a jq-shaped computation; a jq error becomes the bash caller's tagged failure. */
export function orFail<T>(fn: () => T, message: string): T {
  try {
    return fn();
  } catch (error) {
    if (error instanceof JqError) throw new CommandFail([message]);
    throw error;
  }
}

export type RunFlags = { onlyStep: string; activity: string; force: boolean; verify: boolean };

export function parseRunFlags(flags: readonly string[]): RunFlags {
  const out: RunFlags = { onlyStep: "", activity: "", force: false, verify: false };
  for (let i = 0; i < flags.length; i += 1) {
    const flag = flags[i];
    if (flag === "--step" || flag === "--activity") {
      const value = flags[i + 1] ?? "";
      if (value === "") {
        throw new CommandFail([
          `plan-ledger: ${flag} requires ${flag === "--step" ? "an id" : "a label"}`,
        ]);
      }
      if (flag === "--step") out.onlyStep = value;
      else out.activity = value;
      i += 1;
    } else if (flag === "--force") out.force = true;
    else if (flag === "--verify") out.verify = true;
    else throw new CommandFail([`plan-ledger: unknown run flag: ${flag ?? ""}`]);
  }
  if (out.activity !== "" && out.onlyStep === "") {
    throw new CommandFail(["plan-ledger: --activity requires --step"]);
  }
  return out;
}

export type RunContext = RunFlags & {
  doc: string;
  steps: PlanStep[];
  base: string;
  root: string;
  ledgerFile: string;
  cur: string;
  branch: string;
  prior: ReadLedger | undefined;
  existing: Map<string, Json>;
  scope: JsonObject;
  env: HostEnv;
  options: LedgerOptions;
  out: Output;
  timeout: string;
  now: () => string;
};

/** bash `[ -s FILE ]`. */
function nonEmpty(file: string): boolean {
  try {
    return statSync(file).size > 0;
  } catch {
    return false;
  }
}

function need<T>(value: T | undefined, lines: string[]): T {
  if (value === undefined) throw new CommandFail(lines);
  return value;
}

export function prepare(
  doc: string,
  flags: RunFlags,
  options: LedgerOptions,
  out: Output,
): RunContext {
  const env = options.env ?? process.env;
  const cwd = options.cwd ?? process.cwd();
  const parsed = parseSteps(doc, { cwd });
  if (!parsed.ok) throw new CommandFail([parsed.message]);
  const found = projectRoot({ env, cwd });
  const base =
    envValue(env, "PUSH_REVIEW_BASE") ?? (found === undefined ? "main" : baseBranch(found, env));
  const root = need(found, ["plan-ledger: not in a git repo"]);
  const ledgerFile = need(ledgerPath({ ...options, env, cwd }), [
    "plan-ledger: cannot resolve ledger path",
  ]);
  const cur = need(diffSha(root, base, { env }), [`plan-ledger: git diff ${base}...HEAD failed`]);
  // Reachable only if HEAD changes after ledgerPath resolved it; bash exits 2 silently here.
  const branch = need(headBranch({ env, cwd: root }), ["plan-ledger: cannot resolve HEAD branch"]);
  const prior = readLedger(ledgerFile);
  const corrupt = `plan-ledger: corrupt prior ledger at ${ledgerFile}`;
  if (prior === undefined && nonEmpty(ledgerFile)) throw new CommandFail([corrupt]);
  const existing =
    prior === undefined ? new Map<string, Json>() : orFail(() => entriesById(prior.value), corrupt);
  const clock = options.now ?? (() => new Date());
  return {
    ...flags,
    doc,
    steps: parsed.steps,
    base,
    root,
    ledgerFile,
    cur,
    branch,
    prior,
    existing,
    scope: {},
    env,
    options,
    out,
    timeout: envValue(env, "PLAN_LEDGER_STEP_TIMEOUT") ?? "1800",
    now: () => isoSeconds(clock()),
  };
}

export function ledgerDoc(ctx: RunContext, updatedAt: string, steps: Json[]): JsonObject {
  return {
    version: 1,
    branch: ctx.branch,
    base_branch: ctx.base,
    plan_doc: ctx.doc,
    updated_at: updatedAt,
    summary: {},
    next: null,
    steps,
  };
}

export function writeOrFail(ctx: RunContext, ledger: Json, failure: string): void {
  const error = writeLedger(ctx.ledgerFile, ledger);
  if (error !== undefined) throw new CommandFail([error, failure]);
}
