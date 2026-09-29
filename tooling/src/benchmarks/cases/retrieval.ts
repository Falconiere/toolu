/**
 * Deterministic tier: for each query, the full-file read (baseline) vs the
 * ast-grep targeted match (treatment), counted in tokens. No model in the loop —
 * hermetic and CI-safe. A missing or empty target is guarded (saved 0 + note).
 *
 *   bun run tooling/src/benchmarks/cases/retrieval.ts [--queries <tsv>] [--corpus <dir>]
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { PRICING_ID } from "../lib/pricing.ts";
import { writeResult } from "../lib/result.ts";
import { benchRoot, headCommit, resultsDir, today } from "../lib/root.ts";
import { countTokens } from "../lib/tokens.ts";
import type { CountOptions } from "../lib/tokens.ts";

const DEFAULT_QUERIES = resolve(
  import.meta.dir,
  "../../../../benchmarks/cases/retrieval/queries.tsv",
);

type Case = {
  id: string;
  baseline_tokens: number;
  treatment_tokens: number;
  saved: number;
  note?: string;
};

type RetrievalOptions = {
  queries: string;
  corpus: string;
  root: string;
  count: CountOptions;
  outDir: string;
};

/** Integer percentage with shell `$(( ))` semantics (truncates toward zero). */
export function percent(base: number, treat: number): number {
  return base > 0 ? Math.trunc(((base - treat) * 100) / base) : 0;
}

function astGrep(pattern: string, lang: string, file: string): string {
  const res = spawnSync("ast-grep", ["run", "--lang", lang, "--pattern", pattern, file], {
    encoding: "utf8",
  });
  // The shell harness hid a missing ast-grep behind 2>/dev/null and recorded a
  // 100% saving; a measurement that did not happen must not produce a number.
  if (res.error !== undefined)
    throw new Error(`retrieval: ast-grep could not run: ${res.error.message}`);
  return res.stdout;
}

function queries(file: string): string[][] {
  return readFileSync(file, "utf8")
    .split("\n")
    .map((line) => line.split("\t"))
    .filter(([id = ""]) => id !== "" && !id.startsWith("#"));
}

type Measured = { case: Case; modes: string[]; base: number; treat: number; note: string };

function tokenTotals(total: number): Record<string, number> {
  return { input: total, output: 0, cache_read: 0, cache_write: 0, total };
}

/** One query: counted both ways, or guarded when the target is missing or empty. */
async function measure(
  opts: RetrievalOptions,
  [id = "", target = "", pattern = "", lang = ""]: string[],
): Promise<Measured> {
  const file = join(opts.corpus, target);
  if (!existsSync(file) || !statSync(file).isFile() || statSync(file).size === 0) {
    const guarded = {
      id,
      baseline_tokens: 0,
      treatment_tokens: 0,
      saved: 0,
      note: "missing-or-empty-target",
    };
    return { case: guarded, modes: [], base: 0, treat: 0, note: `${id}:missing-target; ` };
  }
  const [base, treat] = await Promise.all([
    countTokens(readFileSync(file, "utf8"), opts.count),
    countTokens(astGrep(pattern, lang, file), opts.count),
  ]);
  const counted = {
    id,
    baseline_tokens: base.tokens,
    treatment_tokens: treat.tokens,
    saved: percent(base.tokens, treat.tokens),
  };
  return {
    case: counted,
    modes: [base.mode, treat.mode],
    base: base.tokens,
    treat: treat.tokens,
    note: "",
  };
}

async function runRetrieval(opts: RetrievalOptions): Promise<string> {
  if (!existsSync(opts.queries))
    throw new Error(`retrieval: queries file not found: ${opts.queries}`);
  const measured = await Promise.all(queries(opts.queries).map((q) => measure(opts, q)));
  const sumBase = measured.reduce((acc, m) => acc + m.base, 0);
  const sumTreat = measured.reduce((acc, m) => acc + m.treat, 0);
  const unique = [...new Set(measured.flatMap((m) => m.modes))].toSorted();
  const mode = unique[0] ?? "heuristic";
  return writeResult(
    {
      mechanism: "retrieval",
      tier: "deterministic",
      method: "tool-bytes",
      tokenizer: { mode, source: mode === "exact" ? "count_tokens" : "bytes-div-4" },
      provenance: {
        model: null,
        date: today(),
        commit: headCommit(opts.root),
        n_runs: 1,
        pricing_id: PRICING_ID,
      },
      baseline: { label: "full-read", tokens: tokenTotals(sumBase), cost: null },
      treatment: { label: "ast-grep", tokens: tokenTotals(sumTreat), cost: null },
      delta: {
        tokens_pct: percent(sumBase, sumTreat),
        cost_pct: null,
        abs_tokens: sumBase - sumTreat,
        mean: null,
        stddev: null,
      },
      cases: measured.map((m) => m.case),
      notes: measured.map((m) => m.note).join(""),
      _modes: unique,
    },
    opts.outDir,
  );
}

/** Parse `--queries` / `--corpus`; null on an unknown argument. */
function parseRetrievalArgs(
  argv: readonly string[],
  root: string,
): { queries: string; corpus: string } | null {
  const out = { queries: DEFAULT_QUERIES, corpus: root };
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i];
    const value = argv[i + 1] ?? "";
    if (flag === "--queries") out.queries = value;
    else if (flag === "--corpus") out.corpus = value;
    else return null;
  }
  return out;
}

export async function retrievalMain(argv: readonly string[]): Promise<number> {
  const root = benchRoot();
  const args = parseRetrievalArgs(argv, root);
  if (args === null) {
    console.error(`retrieval: unknown arg: ${argv.join(" ")}`);
    return 2;
  }
  try {
    const env = process.env;
    const count = { apiKey: env["ANTHROPIC_API_KEY"], baseUrl: env["ANTHROPIC_BASE_URL"] };
    process.stdout.write(
      `${await runRetrieval({ ...args, root, count, outDir: resultsDir(root) })}\n`,
    );
    return 0;
  } catch (err: unknown) {
    console.error(err instanceof Error ? err.message : String(err));
    return 1;
  }
}

if (import.meta.main) process.exitCode = await retrievalMain(process.argv.slice(2));
