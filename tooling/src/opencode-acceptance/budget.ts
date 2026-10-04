/**
 * Startup and per-tool cost that toolu adds on the pinned host (#362).
 *
 * The same scripted session (six `bash` calls) runs four times, alternating a
 * reference project, which loads a no-op plugin so the host's plugin
 * dependency wait applies to both, and a project loading toolu with every
 * catalog plugin. The scripted provider stamps each request on arrival:
 * startup is spawn to the first tool-bearing request, and per-tool time is the
 * median gap between consecutive tool-bearing requests. Each kind keeps its
 * fastest run, and the overheads are toolu's minus the reference's, against
 * the committed ceilings in `contract/acceptance-budgets.json`.
 */
import { join } from "node:path";
import { z } from "zod";
import { runHost } from "../opencode-host/host-run.ts";
import type { Scripts } from "../opencode-host/provider.ts";
import { contractPaths } from "../opencode-host/results.ts";
import {
  GATED_FILES,
  ROOT,
  diagnostics,
  entrySession,
  installShim,
} from "../opencode-host/scenarios-entry.ts";
import { ContractError, readJson } from "../opencode-host/schema.ts";
import { openSession, type ProbeSession } from "../opencode-host/session.ts";
import {
  hostEvidence,
  inSequence,
  type AcceptanceCheck,
  type AcceptanceContext,
} from "./checks.ts";

export type BudgetSample = {
  kind: "reference" | "toolu";
  spawnAt: number;
  requestTimes: readonly number[];
};

const TOOL_CALLS = 6;
const SCRIPTS: Scripts = {
  "budget.tools": [...Array.from({ length: TOOL_CALLS }).keys()].map((n) => ({
    tool: "bash",
    args: { command: `printf ok-${n}`, description: `budget ${n}` },
  })),
};
const allowBash = (): Record<string, unknown> => ({ permission: { bash: "allow" } });
const NOOP_PLUGIN = 'export default { id: "noop", server: async () => ({}) };\n';
const Budgets = z.strictObject({
  version: z.literal(1),
  startupOverheadMs: z.number().positive(),
  perToolOverheadMs: z.number().positive(),
});
const ToolRequest = z.looseObject({ tools: z.array(z.unknown()).min(1) });

function median(values: readonly number[]): number {
  const sorted = values.toSorted((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  const upper = sorted[mid] ?? 0;
  return sorted.length % 2 === 1 ? upper : ((sorted[mid - 1] ?? 0) + upper) / 2;
}

/** Startup and median per-tool time of one run; a run that made too few tool calls is an error. */
function measure(run: BudgetSample): { startup: number; perTool: number } {
  const [first] = run.requestTimes;
  if (first === undefined || run.requestTimes.length < TOOL_CALLS + 1) {
    throw new ContractError(
      `budget ${run.kind} run made ${run.requestTimes.length} tool requests, expected ${TOOL_CALLS + 1}`,
    );
  }
  const gaps = run.requestTimes.slice(1).map((at, n) => at - (run.requestTimes[n] ?? at));
  return { startup: first - run.spawnAt, perTool: median(gaps) };
}

/** Toolu's overheads over the reference, each kind taken at its fastest run. */
export function overheads(samples: readonly BudgetSample[]): {
  startupOverheadMs: number;
  perToolOverheadMs: number;
} {
  const best = (kind: BudgetSample["kind"]): { startup: number; perTool: number } => {
    const runs = samples.filter((item) => item.kind === kind).map(measure);
    if (runs.length === 0) throw new ContractError(`budget has no ${kind} run`);
    return {
      startup: Math.min(...runs.map((item) => item.startup)),
      perTool: Math.min(...runs.map((item) => item.perTool)),
    };
  };
  const toolu = best("toolu");
  const reference = best("reference");
  return {
    startupOverheadMs: Math.round(toolu.startup - reference.startup),
    perToolOverheadMs: Math.round(toolu.perTool - reference.perTool),
  };
}

function budgetSession(ctx: AcceptanceContext, kind: BudgetSample["kind"]): ProbeSession {
  if (kind === "reference") {
    return openSession(ctx.cacheRoot, {
      config: allowBash,
      scripts: SCRIPTS,
      files: { ".opencode/plugins/noop.ts": NOOP_PLUGIN },
    });
  }
  const s = entrySession(ctx, { config: allowBash, scripts: SCRIPTS, files: GATED_FILES });
  installShim(s);
  s.env.TOOLU_REPO_ROOT = ROOT;
  return s;
}

async function timedRun(
  ctx: AcceptanceContext,
  kind: BudgetSample["kind"],
): Promise<BudgetSample & { ready: number }> {
  using s = budgetSession(ctx, kind);
  const spawnAt = Date.now();
  const run = await runHost(ctx.bin, s, ["--print-logs", "PROBE:budget.tools"]);
  const requestTimes = s
    .requests()
    .filter((req) => ToolRequest.safeParse(req.body).success)
    .map((req) => req.at);
  return {
    kind,
    spawnAt,
    requestTimes,
    ready: diagnostics(run.stderr, "toolu: ready (16 plugins"),
  };
}

async function budgetRun(ctx: AcceptanceContext) {
  const ceilings = readJson(join(contractPaths().dir, "acceptance-budgets.json"), Budgets);
  const kinds: BudgetSample["kind"][] = ["reference", "toolu", "reference", "toolu"];
  const samples = await inSequence(kinds, (kind) => timedRun(ctx, kind));
  const measured = overheads(samples);
  const allReady = samples.every((item) => item.kind === "reference" || item.ready === 1);
  const pass =
    allReady &&
    measured.startupOverheadMs <= ceilings.startupOverheadMs &&
    measured.perToolOverheadMs <= ceilings.perToolOverheadMs;
  const observed = {
    startupOverheadMs: {
      measured: measured.startupOverheadMs,
      ceiling: ceilings.startupOverheadMs,
    },
    perToolOverheadMs: {
      measured: measured.perToolOverheadMs,
      ceiling: ceilings.perToolOverheadMs,
    },
    tooluReady: allReady,
    samples: samples.map(({ kind, spawnAt, requestTimes }) => ({
      kind,
      requestOffsetsMs: requestTimes.map((at) => at - spawnAt),
    })),
  };
  return { pass, observed };
}

export const BUDGET_CHECK: AcceptanceCheck = {
  id: "budget.overhead",
  family: "budget",
  plugins: ["toolu"],
  evidence: hostEvidence(),
  run: budgetRun,
};
