/**
 * Live headless A/B for the whole-session AGGREGATE. For each task, BASELINE
 * runs `claude --bare` (toolu off: no plugins or hooks) and TREATMENT runs
 * `claude` with toolu on. Compares total cost (the JSON result's
 * total_cost_usd) and total tokens (the usage rollup over the pinned session's
 * transcript and its subagent transcripts). NOT the sum of per-mechanism
 * deltas. A run whose `claude` call fails, or whose transcript is missing or
 * empty, is dropped with a note; no surviving pair aborts non-zero.
 *
 *   bun run tooling/src/benchmarks/cases/whole-session.ts [--n 5] [--model <id>] [--tasks <dir>]
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { get } from "../../json-path.ts";
import { PRICING_ID } from "../lib/pricing.ts";
import { writeResult } from "../lib/result.ts";
import { benchRoot, headCommit, resultsDir, today } from "../lib/root.ts";
import { DEFAULT_MODEL, captured, stats } from "../lib/tokens.ts";
import { usageRollup } from "../lib/usage.ts";
import { percent } from "./retrieval.ts";

const DEFAULT_TASKS = resolve(import.meta.dir, "../../../../benchmarks/cases/whole-session/tasks");

type Sample = { tokens: number; cost: number };
type WholeSessionOptions = {
  n: number;
  model: string;
  tasks: string;
  cwd: string;
  configDir: string;
};

/** Where Claude Code writes a pinned session: <config>/projects/<slug>/<session>.jsonl. */
function transcriptPath(configDir: string, cwd: string, session: string): string {
  return join(configDir, "projects", cwd.replace(/[^A-Za-z0-9]/g, "-"), `${session}.jsonl`);
}

function transcriptFiles(transcript: string): string[] {
  const subagents = join(transcript.slice(0, -".jsonl".length), "subagents");
  const agents = existsSync(subagents)
    ? readdirSync(subagents)
        .filter((f) => f.startsWith("agent-") && f.endsWith(".jsonl"))
        .toSorted()
        .map((f) => join(subagents, f))
    : [];
  return [transcript, ...agents];
}

/** One headless run; null when it must be dropped. */
function runOne(opts: WholeSessionOptions, task: string, bare: boolean): Sample | null {
  const session = crypto.randomUUID().toLowerCase();
  const args = [
    "-p",
    ...(bare ? ["--bare"] : []),
    "--output-format",
    "json",
    "--model",
    opts.model,
    "--session-id",
    session,
    task,
  ];
  // Run from cwd so the transcript slug matches where Claude Code actually writes.
  const res = spawnSync("claude", args, { cwd: opts.cwd, encoding: "utf8" });
  if (res.status !== 0) return null;
  let cost: unknown;
  try {
    cost = get(JSON.parse(res.stdout), "total_cost_usd");
  } catch {
    return null;
  }
  if (typeof cost !== "number") return null;
  const transcript = transcriptPath(opts.configDir, opts.cwd, session);
  if (!existsSync(transcript) || statSync(transcript).size === 0) return null;
  const roll = usageRollup(transcriptFiles(transcript));
  if (roll.messages <= 0) return null;
  return { tokens: roll.totals.tokens, cost };
}

type Totals = {
  base: number;
  treat: number;
  baseCost: number;
  treatCost: number;
  tokSamples: number[];
  costSamples: number[];
  notes: string;
};

function runTask(opts: WholeSessionOptions, file: string, totals: Totals): Record<string, unknown> {
  const id = basename(file, ".txt");
  const task = captured(readFileSync(file, "utf8"));
  const side = { base: 0, treat: 0, baseRuns: 0, treatRuns: 0 };
  for (let run = 1; run <= opts.n; run += 1) {
    const b = runOne(opts, task, true);
    const t = runOne(opts, task, false);
    if (b === null) totals.notes += `${id}#${String(run)}:baseline-dropped; `;
    else [side.base, side.baseRuns] = [side.base + b.tokens, side.baseRuns + 1];
    if (t === null) totals.notes += `${id}#${String(run)}:treatment-dropped; `;
    else [side.treat, side.treatRuns] = [side.treat + t.tokens, side.treatRuns + 1];
    if (b !== null && t !== null) {
      totals.base += b.tokens;
      totals.treat += t.tokens;
      totals.tokSamples.push(t.tokens);
      totals.costSamples.push(t.cost);
      totals.baseCost += b.cost;
      totals.treatCost += t.cost;
    }
  }
  return {
    id,
    baseline_tokens: side.base,
    treatment_tokens: side.treat,
    baseline_runs: side.baseRuns,
    treatment_runs: side.treatRuns,
  };
}

function runWholeSession(opts: WholeSessionOptions, root: string, outDir: string): string {
  if (!existsSync(opts.tasks)) throw new Error(`whole-session: tasks dir not found: ${opts.tasks}`);
  const totals: Totals = {
    base: 0,
    treat: 0,
    baseCost: 0,
    treatCost: 0,
    tokSamples: [],
    costSamples: [],
    notes: "",
  };
  const files = readdirSync(opts.tasks)
    .filter((f) => f.endsWith(".txt"))
    .toSorted()
    .map((f) => join(opts.tasks, f));
  const cases = files.map((file) => runTask(opts, file, totals));
  if (totals.tokSamples.length === 0) {
    throw new Error(
      "whole-session: no surviving paired samples (all runs dropped); refusing to write",
    );
  }
  return writeResult(liveResult(opts, root, totals, cases), outDir);
}

function liveTokens(total: number): Record<string, number | null> {
  return { input: null, output: null, cache_read: null, cache_write: null, total };
}

function liveResult(
  opts: WholeSessionOptions,
  root: string,
  totals: Totals,
  cases: ReadonlyArray<Record<string, unknown>>,
): Record<string, unknown> {
  const costStats = stats(totals.costSamples);
  const { base, treat, baseCost, treatCost } = totals;
  const provenance = {
    model: opts.model,
    date: today(),
    commit: headCommit(root),
    pricing_id: PRICING_ID,
  };
  return {
    mechanism: "whole-session",
    tier: "live",
    method: "headless-ab",
    tokenizer: { mode: "usage", source: "usage" },
    provenance: { ...provenance, n_runs: totals.tokSamples.length },
    baseline: { label: "toolu-off", tokens: liveTokens(base), cost: baseCost },
    treatment: { label: "toolu-on", tokens: liveTokens(treat), cost: treatCost },
    delta: {
      tokens_pct: percent(base, treat),
      cost_pct: baseCost > 0 ? ((baseCost - treatCost) * 100) / baseCost : null,
      abs_tokens: base - treat,
      mean: costStats.mean,
      stddev: costStats.stddev,
    },
    token_stats: stats(totals.tokSamples),
    cost_stats: costStats,
    cases,
    notes: `${totals.notes}AGGREGATE whole-session delta (toolu-on vs --bare); NOT the sum of per-mechanism deltas.`,
    _modes: ["usage", "usage"],
  };
}

export function wholeSessionMain(argv: readonly string[]): number {
  const root = benchRoot();
  const env = process.env;
  const configDir = env["CLAUDE_CONFIG_DIR"] ?? join(env["HOME"] ?? "", ".claude");
  const opts: WholeSessionOptions = {
    n: 5,
    model: DEFAULT_MODEL,
    tasks: DEFAULT_TASKS,
    cwd: root,
    configDir,
  };
  // Arguments first: an unknown one is a hard 2 whether or not the CLI is installed.
  for (let i = 0; i < argv.length; i += 2) {
    const value = argv[i + 1] ?? "";
    if (argv[i] === "--n") opts.n = Number(value);
    else if (argv[i] === "--model") opts.model = value;
    else if (argv[i] === "--tasks") opts.tasks = value;
    else {
      console.error(`whole-session: unknown arg: ${argv[i] ?? ""}`);
      return 2;
    }
  }
  if (Bun.which("claude") === null) {
    console.error("whole-session: live tier needs the claude CLI");
    return 1;
  }
  try {
    process.stdout.write(`${runWholeSession(opts, root, resultsDir(root))}\n`);
    return 0;
  } catch (err: unknown) {
    console.error(err instanceof Error ? err.message : String(err));
    return 1;
  }
}

if (import.meta.main) process.exitCode = wholeSessionMain(process.argv.slice(2));
