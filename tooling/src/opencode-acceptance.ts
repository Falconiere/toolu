#!/usr/bin/env bun
/**
 * Real OpenCode acceptance (#362): `bun run test:opencode [--report <file>] [--only <id>…]`.
 *
 * Runs every acceptance check on the pinned `opencode-ai` host in isolated
 * profiles against the scripted loopback model, then writes the JSON report
 * (default `$TMPDIR/opencode-acceptance-<platform>.json`) and a Markdown
 * summary, also appended to `$GITHUB_STEP_SUMMARY` when set. A complete run
 * exits 0 only on acceptance. `--only` narrows the run for local work: its
 * report is never acceptance, and its exit code covers only the selected checks.
 */
import { appendFileSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { ContractError } from "./opencode-host/schema.ts";
import { markdownSummary, selectedPass } from "./opencode-acceptance/report.ts";
import { runAcceptance } from "./opencode-acceptance/run.ts";

type Args = { report: string; only: string[] };

/** `--report <file>` and `--only <id>…`; ids without `--only`, or `--only` without ids, are errors. */
export function parseAcceptanceArgs(argv: readonly string[]): Args {
  const { values, positionals } = parseArgs({
    args: [...argv],
    options: { report: { type: "string" }, only: { type: "boolean" } },
    allowPositionals: true,
  });
  if (values.only === true && positionals.length === 0)
    throw new ContractError("--only needs at least one check id");
  if (values.only !== true && positionals.length > 0)
    throw new ContractError(`unexpected arguments: ${positionals.join(" ")} (use --only)`);
  const fallback = join(tmpdir(), `opencode-acceptance-${process.platform}-${process.arch}.json`);
  return { report: resolve(values.report ?? fallback), only: positionals };
}

async function main(argv: readonly string[]): Promise<number> {
  const args = parseAcceptanceArgs(argv);
  const report = await runAcceptance({ only: args.only });
  mkdirSync(dirname(args.report), { recursive: true });
  writeFileSync(args.report, `${JSON.stringify(report, null, 2)}\n`);
  const summary = markdownSummary(report);
  process.stdout.write(`\n${summary}\nopencode-acceptance: report ${args.report}\n`);
  const stepSummary = process.env.GITHUB_STEP_SUMMARY;
  if (stepSummary !== undefined && stepSummary !== "") appendFileSync(stepSummary, `${summary}\n`);
  if (report.complete) return report.pass ? 0 : 1;
  return selectedPass(report) ? 0 : 1;
}

if (import.meta.main) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (err: unknown) {
    if (!(err instanceof ContractError)) throw err;
    process.stderr.write(`opencode-acceptance: ${err.message}\n`);
    process.exitCode = 1;
  }
}
