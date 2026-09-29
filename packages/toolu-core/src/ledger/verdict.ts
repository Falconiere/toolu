/**
 * `verdict.sh` (#256): one read-only view over the four push gates (quality,
 * plan, review and docs) that answers "is this branch done, and why not". It
 * never writes state and never runs a check. Exit 0 means ready, 1 blocked
 * (a gate failed or escalated), 2 an error.
 */
import { gitToplevel } from "../host/host-roots.ts";
import { envValue } from "../host/host-name.ts";
import { diffSha } from "../state/diff-sha.ts";
import { baseBranch, currentBranch, hasGit } from "../state/state-git.ts";
import { isoSeconds, toJqJson } from "../state/state-io.ts";
import { Output, type CommandResult, type LedgerOptions } from "./ledger-io.ts";
import { planGate, qualityGate, type Gate, type GateContext } from "./verdict-gates.ts";
import { docsGate, reviewGate } from "./verdict-review.ts";

export type VerdictReport = {
  version: 1;
  branch: string;
  base_branch: string;
  diff_sha: string;
  overall: "ready" | "blocked";
  gates: { quality: Gate; plan: Gate; review: Gate; docs: Gate };
  generated_at: string;
};

/** Every gate for the repo at `cwd`, or the error line when there is no repo to judge. */
export function verdictReport(
  options: LedgerOptions = {},
  warn: (line: string) => void = () => {},
): { report: VerdictReport } | { error: string } {
  const env = options.env ?? process.env;
  const cwd = options.cwd ?? process.cwd();
  if (!hasGit(env)) return { error: "verdict: git is required" };
  const root = gitToplevel(env, cwd);
  if (root === undefined) return { error: "verdict: not a git repository" };
  const branch = currentBranch(root, env);
  const base = envValue(env, "PUSH_REVIEW_BASE") ?? baseBranch(root, env);
  const cur = diffSha(root, base, { env }) ?? "";
  const ctx: GateContext = { root, branch, base, cur, env, cwd, warn };
  if (options.host !== undefined) ctx.host = options.host;
  const gates = {
    quality: qualityGate(ctx),
    plan: planGate(ctx),
    review: reviewGate(ctx),
    docs: docsGate(ctx),
  };
  const blocked = Object.values(gates).some((g) => g.state === "fail" || g.state === "escalate");
  const report: VerdictReport = {
    version: 1,
    branch,
    base_branch: base,
    diff_sha: cur,
    overall: blocked ? "blocked" : "ready",
    gates,
    generated_at: isoSeconds(options.now?.() ?? new Date()),
  };
  return { report };
}

/** `printf '%-8s %-9s %s\n'`. */
function row(gate: string, state: string, reason: string): string {
  return `${gate.padEnd(8)} ${state.padEnd(9)} ${reason}\n`;
}

/** `vd_render_status`: the human table (same data as `json`). */
export function renderVerdictStatus(report: VerdictReport): string {
  let out = `verdict: ${report.branch} vs ${report.base_branch} (diff ${report.diff_sha.slice(0, 12)})\n`;
  out += row("GATE", "STATE", "REASON");
  for (const name of ["quality", "plan", "review", "docs"] as const) {
    out += row(name, report.gates[name].state, report.gates[name].reason);
  }
  return `${out}overall: ${report.overall}\n`;
}

/** `verdict.sh status|json`. */
export function verdictMain(argv: readonly string[], options: LedgerOptions = {}): CommandResult {
  const out = new Output(options.onStderr);
  const mode = argv[0] ?? "";
  if (mode !== "status" && mode !== "json") {
    out.stderr("verdict: usage: status | json");
    return out.result(2);
  }
  const result = verdictReport(options, (line) => out.stderr(line));
  if ("error" in result) {
    out.stderr(result.error);
    return out.result(2);
  }
  const { report } = result;
  out.stdout(mode === "json" ? `${toJqJson(report, true)}\n` : renderVerdictStatus(report));
  return out.result(report.overall === "ready" ? 0 : 1);
}
