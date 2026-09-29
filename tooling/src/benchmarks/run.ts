/**
 * Benchmarks entry point (`bun run benchmarks`): dispatches per-mechanism cases
 * by tier. The deterministic tier (retrieval) is hermetic and CI-safe; the live
 * tier (whole-session) needs the claude CLI and runs by hand.
 */
import { retrievalMain } from "./cases/retrieval.ts";
import { wholeSessionMain } from "./cases/whole-session.ts";
import { ResultError, validateResult } from "./lib/result.ts";

const USAGE = `usage: bun run benchmarks --tier <deterministic|live|both> [--mechanism <name|all>]
       bun run benchmarks --validate <result.json>

mechanisms:
  retrieval                         deterministic tier (hermetic, CI)
  whole-session                     live tier (manual; needs API key / claude CLI)
  all                               every mechanism for the selected tier
`;

type Args = { tier: string; mechanism: string; validate: string; help: boolean };

function parse(argv: readonly string[]): Args | string {
  const args: Args = { tier: "both", mechanism: "all", validate: "", help: false };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i] ?? "";
    if (arg === "-h" || arg === "--help") return { ...args, help: true };
    if (arg !== "--tier" && arg !== "--mechanism" && arg !== "--validate")
      return `unknown arg: ${arg}`;
    const value = argv[i + 1] ?? "";
    i += 1;
    if (arg === "--tier") args.tier = value;
    else if (arg === "--mechanism") args.mechanism = value;
    else args.validate = value;
  }
  return args;
}

async function main(argv: readonly string[]): Promise<number> {
  const args = parse(argv);
  if (typeof args === "string") {
    process.stderr.write(`benchmarks: ${args}\n${USAGE}`);
    return 2;
  }
  if (args.help) {
    process.stdout.write(USAGE);
    return 0;
  }
  if (args.validate !== "") {
    try {
      validateResult(args.validate);
      return 0;
    } catch (err: unknown) {
      if (!(err instanceof ResultError)) throw err;
      console.error(`bench_result: ${err.message}`);
      return 1;
    }
  }
  if (!["deterministic", "live", "both"].includes(args.tier)) {
    console.error(`benchmarks: bad --tier: ${args.tier}`);
    return 2;
  }
  let rc = 0;
  if (args.tier !== "live" && ["all", "retrieval"].includes(args.mechanism)) {
    if ((await retrievalMain([])) !== 0) rc = 1;
  }
  if (args.tier !== "deterministic") {
    const live =
      args.mechanism === "all" ? ["whole-session"] : args.mechanism.split(/\s+/).filter(Boolean);
    for (const mechanism of live) {
      if (mechanism === "whole-session") {
        if (wholeSessionMain([]) !== 0) rc = 1;
      } else if (mechanism !== "retrieval") {
        console.error(`benchmarks: unknown mechanism: ${mechanism}`);
        rc = 2;
      }
    }
  }
  return rc;
}

if (import.meta.main) process.exitCode = await main(process.argv.slice(2));
