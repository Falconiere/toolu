/**
 * The remaining `plan-ledger.sh` commands (#256): `status` (`pl_cmd_status`),
 * `preflight` (`pl_cmd_preflight`), `path`, `root` and `--self-test`, plus
 * `ledgerMain`, the CLI dispatch with the same usage line and exit codes.
 */
import { accessSync, constants, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { envValue, type HostName } from "../host/host-name.ts";
import { resolveHost } from "../host/host-roots.ts";
import { diffSha } from "../state/diff-sha.ts";
import { baseBranch, branchSlug, hasGit } from "../state/state-git.ts";
import { toJqJson } from "../state/state-io.ts";
import {
  Output,
  headBranch,
  ledgerPath,
  projectRoot,
  readLedger,
  writeLedger,
  type CommandResult,
  type LedgerOptions,
} from "./ledger-io.ts";
import { JqError, alt, get, raw, type Json } from "./ledger-jq.ts";
import {
  acCoverage,
  allFresh,
  healOrphans,
  orphanCutoff,
  recompute,
  summaryLine,
} from "./ledger-model.ts";
import { docField, isFile, isSpecless, parseSteps } from "./ledger-parse.ts";
import { CommandFail, orFail } from "./ledger-run-context.ts";
import { ledgerRun } from "./ledger-run.ts";

const DEFAULT_STUCK_SECONDS = 300;

/** `PL_STUCK_THRESHOLD` (seconds, default 300): how old a `running` step must be to count as orphaned. */
function stuckThreshold(options: LedgerOptions): number {
  const value = envValue(options.env ?? process.env, "PL_STUCK_THRESHOLD");
  return value !== undefined && /^-?\d+$/.test(value) ? Number(value) : DEFAULT_STUCK_SECONDS;
}

function cutoff(options: LedgerOptions): string {
  return orphanCutoff(options.now?.() ?? new Date(), stuckThreshold(options));
}

/** `PUSH_REVIEW_BASE`, else origin's HEAD branch, else `main`. */
function baseFor(options: LedgerOptions, root: string | undefined): string {
  const env = options.env ?? process.env;
  return envValue(env, "PUSH_REVIEW_BASE") ?? (root === undefined ? "main" : baseBranch(root, env));
}

/** bash `[ -f P ] || { [ -n "$root" ] && [ -f "$root/P" ] && P="$root/P"; }`, resolved from `cwd`. */
function fileOrUnderRoot(path: string, cwd: string, root: string | undefined): string | undefined {
  if (isFile(resolve(cwd, path))) return resolve(cwd, path);
  if (root !== undefined && isFile(join(root, path))) return join(root, path);
  return undefined;
}

/** The report-only AC-coverage section of `status`, resolved from the ledger's plan doc. */
function coverageReport(ledger: Json, cur: string, options: LedgerOptions, out: Output): void {
  const cwd = options.cwd ?? process.cwd();
  const planDoc = raw(alt(get(ledger, "plan_doc"), ""));
  if (planDoc === "") return;
  const root = projectRoot(options);
  const plan = fileOrUnderRoot(planDoc, cwd, root);
  if (plan === undefined) return;
  const specField = docField(plan, "Spec");
  const spec = isSpecless(specField)
    ? specField
    : (fileOrUnderRoot(specField, cwd, root) ?? resolve(cwd, specField));
  const report = acCoverage(ledger, cur, spec);
  out.stdout(report.stdout);
  for (const line of report.stderr) out.stderr(line);
}

/** `plan-ledger.sh status`: heal, recompute and rewrite the ledger without running checks. */
export function ledgerStatus(options: LedgerOptions = {}): CommandResult {
  const out = new Output(options.onStderr);
  try {
    const env = options.env ?? process.env;
    const base = baseFor(options, projectRoot(options));
    const file = ledgerPath(options);
    if (file === undefined) throw new CommandFail(["plan-ledger: cannot resolve ledger path"]);
    const read = readLedger(file);
    if (read === undefined) throw new CommandFail([`plan-ledger: no ledger at ${file}`]);
    const cur = diffSha(options.cwd ?? process.cwd(), base, { env });
    if (cur === undefined) throw new CommandFail([`plan-ledger: git diff ${base}...HEAD failed`]);
    const branch = headBranch(options);
    // Reachable only if HEAD changes after ledgerPath resolved it; bash exits 2 silently here.
    if (branch === undefined) throw new CommandFail(["plan-ledger: cannot resolve HEAD branch"]);
    const healed = orFail(
      () => healOrphans(read.value, cutoff(options)),
      "plan-ledger: failed to heal orphaned running steps",
    );
    const ledger = orFail(() => recompute(healed, cur), "plan-ledger: failed to recompute summary");
    const error = writeLedger(file, ledger);
    if (error !== undefined) throw new CommandFail([error, "plan-ledger: ledger write failed"]);
    try {
      out.stdout(`${summaryLine(ledger, branchSlug(branch))}\n`);
    } catch (failure) {
      if (!(failure instanceof JqError)) throw failure;
    }
    coverageReport(ledger, cur, options, out);
    return out.result(allFresh(ledger) ? 0 : 1);
  } catch (error) {
    if (!(error instanceof CommandFail)) throw error;
    for (const line of error.lines) out.stderr(line);
    return out.result(2);
  }
}

/** bash `[ -r PATH ]`. */
function readable(path: string): boolean {
  try {
    accessSync(path, constants.R_OK);
    return true;
  } catch {
    return false;
  }
}

const isApproved = (status: string): boolean =>
  status.replace(/[A-Z]/g, (c) => c.toLowerCase()) === "approved";

/** Heal the branch ledger in place (when it changes), and return its `plan_doc` for a no-arg preflight. */
function healLedgerFor(options: LedgerOptions): string {
  const file = ledgerPath(options);
  const read = file === undefined ? undefined : readLedger(file);
  if (file === undefined || read === undefined) return "";
  try {
    const healed = healOrphans(read.value, cutoff(options));
    if (toJqJson(healed, true) !== read.text) writeLedger(file, healed);
  } catch (error) {
    if (!(error instanceof JqError)) throw error;
  }
  try {
    return raw(alt(get(read.value, "plan_doc"), ""));
  } catch (error) {
    if (!(error instanceof JqError)) throw error;
    return "";
  }
}

/** Where a refused preflight sends the agent: OpenCode loads the generated skill by its id. */
function reviewRemedy(host: HostName, phase: "plan" | "spec"): string {
  const action =
    host === "opencode"
      ? 'load skill({ name: "delivery-flow-delivery-flow" })'
      : host === "claude" || host === "codex"
        ? "run /delivery-flow:delivery-flow"
        : "load the delivery-flow skill";
  return `${action} (${phase} review phase)`;
}

function preflightChecks(
  plan: string,
  root: string | undefined,
  cwd: string,
  host: HostName,
): { code: 0 | 1 | 2; line?: string } {
  const under = (path: string): string =>
    isAbsolute(path) || root === undefined ? resolve(cwd, path) : join(root, path);
  const planAbs = under(plan);
  if (!readable(planAbs))
    return { code: 2, line: `preflight: plan doc not found or unreadable: ${plan}` };
  const status = docField(planAbs, "Status");
  const planReview = reviewRemedy(host, "plan");
  if (status === "")
    return {
      code: 1,
      line: `preflight: plan has no **Status:** header (${plan}) — ${planReview} to stamp it`,
    };
  if (!isApproved(status))
    return { code: 1, line: `preflight: plan not approved (Status: ${status}) — ${planReview}` };
  const spec = docField(planAbs, "Spec");
  if (isSpecless(spec)) return { code: 0 };
  const specAbs = under(spec);
  if (!readable(specAbs))
    return { code: 1, line: `preflight: declared spec not found or unreadable: ${spec}` };
  const specStatus = docField(specAbs, "Status");
  if (isApproved(specStatus)) return { code: 0 };
  const shown = specStatus === "" ? "none" : specStatus;
  return {
    code: 1,
    line: `preflight: spec ${spec} not approved (Status: ${shown}) — ${reviewRemedy(host, "spec")}`,
  };
}

/**
 * `plan-ledger.sh preflight [PLAN]`: exit 0 only when the plan is Approved
 * and its declared spec (if any) is Approved. It also heals orphaned
 * `running` steps in the branch ledger.
 */
export function ledgerPreflight(plan = "", options: LedgerOptions = {}): CommandResult {
  const out = new Output(options.onStderr);
  const root = projectRoot(options);
  const fromLedger = healLedgerFor(options);
  const target = plan === "" ? fromLedger : plan;
  if (target === "") {
    out.stderr("preflight: no plan doc given and no ledger plan_doc to resolve");
    return out.result(2);
  }
  const { host } = resolveHost(options);
  const verdict = preflightChecks(target, root, options.cwd ?? process.cwd(), host);
  if (verdict.line !== undefined) out.stderr(verdict.line);
  return out.result(verdict.code);
}

const SELF_TEST_DOC = `# Self-test Plan

## Steps (machine-readable)

\`\`\`json
[
  { "id": "s1", "title": "ok", "check": "true" },
  { "id": "s2", "title": "fail", "check": "false" }
]
\`\`\`
`;

/** `plan-ledger.sh --self-test`: parse a two-step fixture and check the result. */
export function ledgerSelfTest(options: LedgerOptions = {}): CommandResult {
  const out = new Output(options.onStderr);
  const dir = mkdtempSync(join(tmpdir(), "plan-ledger-self-test-"));
  const doc = join(dir, "selftest-plan.md");
  writeFileSync(doc, SELF_TEST_DOC);
  const parsed = parseSteps(doc);
  rmSync(dir, { recursive: true, force: true });
  if (!parsed.ok) {
    out.stderr(parsed.message);
    out.stderr("plan-ledger --self-test: parse failed");
    return out.result(1);
  }
  const [first, second] = parsed.steps;
  if (parsed.steps.length !== 2 || first?.id !== "s1" || second?.check !== "false") {
    out.stderr("plan-ledger --self-test: unexpected parse result");
    return out.result(1);
  }
  out.stdout("plan-ledger --self-test: ok\n");
  return out.result(0);
}

const USAGE =
  "plan-ledger: usage: run <doc> [--step <id>] [--activity <label>] | status | preflight [<doc>] | path | root | --self-test";

function fail(line: string, options: LedgerOptions): CommandResult {
  const out = new Output(options.onStderr);
  out.stderr(line);
  return out.result(2);
}

function printed(value: string, options: LedgerOptions): CommandResult {
  const out = new Output(options.onStderr);
  out.stdout(`${value}\n`);
  return out.result(0);
}

/** `plan-ledger.sh <command> ...`: the CLI dispatch. */
export async function ledgerMain(
  argv: readonly string[],
  options: LedgerOptions = {},
): Promise<CommandResult> {
  if (!hasGit(options.env ?? process.env)) return fail("plan-ledger: git is required", options);
  const [command = "", ...rest] = argv;
  switch (command) {
    case "run": {
      const [doc = "", ...flags] = rest;
      if (doc === "") return fail("plan-ledger: run requires a plan doc path", options);
      return ledgerRun(doc, flags, options);
    }
    case "status":
      return ledgerStatus(options);
    case "preflight":
      return ledgerPreflight(rest[0] ?? "", options);
    case "path": {
      const path = ledgerPath(options);
      return path === undefined
        ? fail("plan-ledger: cannot resolve ledger path", options)
        : printed(path, options);
    }
    case "root": {
      const root = projectRoot(options);
      return root === undefined
        ? fail("plan-ledger: not in a git repo", options)
        : printed(root, options);
    }
    case "--self-test":
      return ledgerSelfTest(options);
    default:
      return fail(USAGE, options);
  }
}
