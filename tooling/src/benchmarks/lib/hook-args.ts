/** Command-line flags of `bun run bench:hooks` (#410). */
import { resolve } from "node:path";
import { BenchError } from "./hook-data.ts";

export type BenchArgs = {
  runs: number;
  warmup: number;
  only: string[];
  out: string | undefined;
  assert: boolean;
  manifest: string;
  budgets: string;
  payloads: string;
};

const USAGE =
  "usage: bench:hooks [--runs N] [--warmup N] [--only <plugin/entry>]... [--out FILE] " +
  "[--assert] [--manifest FILE] [--budgets FILE] [--payloads FILE]";

function count(flag: string, value: string, min: number): number {
  const n = Number(value);
  if (!Number.isInteger(n) || n < min) {
    throw new BenchError(`${flag} takes an integer >= ${String(min)}, got ${value}\n${USAGE}`);
  }
  return n;
}

/** Parse `argv`; paths default under `root`. */
export function parseArgs(argv: readonly string[], root: string): BenchArgs {
  const args: BenchArgs = {
    runs: 10,
    warmup: 2,
    only: [],
    out: undefined,
    assert: false,
    manifest: resolve(root, "fixtures/rust-ported.json"),
    budgets: resolve(root, "benchmarks/hook-budgets.json"),
    payloads: resolve(root, "benchmarks/cases/hooks/payloads.json"),
  };
  for (let at = 0; at < argv.length; at += 1) {
    const flag = argv[at] ?? "";
    if (flag === "--assert") {
      args.assert = true;
      continue;
    }
    const value = argv[at + 1];
    if (value === undefined) throw new BenchError(`${flag} needs a value\n${USAGE}`);
    at += 1;
    if (flag === "--runs") args.runs = count(flag, value, 1);
    else if (flag === "--warmup") args.warmup = count(flag, value, 0);
    else if (flag === "--only") args.only.push(value);
    else if (flag === "--out") args.out = resolve(value);
    else if (flag === "--manifest") args.manifest = resolve(value);
    else if (flag === "--budgets") args.budgets = resolve(value);
    else if (flag === "--payloads") args.payloads = resolve(value);
    else throw new BenchError(`unknown flag ${flag}\n${USAGE}`);
  }
  return args;
}
