/**
 * ast-grep hook latency (#268, AC-8; epic #247 budget: p50 no worse than the
 * bash baseline + 5 ms on the same machine). For a slice of the golden cases
 * it times toolu's committed pre-tools and post-tools bundles dispatching the
 * pre-port bash modules (extracted from git at the golden's base commit)
 * against the same bundles running the TypeScript modules in process. Each
 * side has its own identically prepared sandbox; runs alternate, so a load
 * change hits both, after warm-up runs on each.
 *
 * Usage: bun run tooling/src/benchmarks/ast-grep-latency.ts [--runs N] [--assert]
 */
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { toStdin } from "@toolu/conformance/harness/fixtures";
import { runPostBundle } from "@toolu/conformance/harness/posttool";
import { pretoolEnv, runBundle } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import type { RunResult } from "@toolu/conformance/harness/spawn";
import { latencyOf, type Latency } from "@toolu/conformance/harness/timing";
import { NUDGE_CASES } from "../../../plugins/ast-grep/hooks/src/__tests__/cases-nudge.ts";
import { SAVINGS_CASES } from "../../../plugins/ast-grep/hooks/src/__tests__/cases-savings.ts";
import {
  BASH_BASE,
  baseSandbox,
  extractBase,
  registerAstGrep,
  type Registration,
} from "../../../plugins/ast-grep/hooks/src/__tests__/golden-sandbox.ts";

const BUDGET_MS = 5;
const WARMUP = 2;
const NUDGE_SLICE = [
  "Bash structural grep",
  "bats: Grep with structural pattern nudges to ast-grep",
  "bats: Bash without grep is silent",
];
const SAVINGS_SLICE = ["bats: ast-grep Bash result bytes, full 0"];

type Row = { name: string; bash: Latency; ts: Latency };
type Call = (sb: Sandbox) => Promise<RunResult>;

function nudgeCall(name: string): Call {
  const c = NUDGE_CASES.find((entry) => entry.name === name);
  if (c === undefined) throw new Error(`no search-nudge case named ${name}`);
  const fixture = {
    kind: "tool",
    event: "PreToolUse",
    toolName: c.toolName,
    toolInput: c.toolInput,
  } as const;
  return (sb) =>
    runBundle({
      cwd: sb.project,
      env: pretoolEnv(sb, "claude"),
      stdin: JSON.stringify(toStdin("claude", fixture, { cwd: sb.project })),
    });
}

function savingsCall(name: string): Call {
  const c = SAVINGS_CASES.find((entry) => entry.name === name);
  if (c === undefined) throw new Error(`no byte-savings case named ${name}`);
  return (sb) =>
    runPostBundle(sb, {
      cwd: sb.project,
      env: pretoolEnv(sb, "claude"),
      stdin: JSON.stringify({ cwd: sb.project, hook_event_name: "PostToolUse", ...c.payload(sb) }),
    });
}

async function row(name: string, call: Call, runs: number, pluginRoot: string): Promise<Row> {
  using bashBox = createSandbox({ git: true });
  using tsBox = createSandbox({ git: true });
  const sides: [Sandbox, Registration, number[]][] = [
    [bashBox, { kind: "bash", pluginRoot }, []],
    [tsBox, { kind: "bundle" }, []],
  ];
  await Promise.all(
    sides.map(([sb, reg]) => {
      baseSandbox(sb);
      return registerAstGrep(sb, "claude", reg);
    }),
  );
  // Sequential by design: concurrent spawns would contend and skew each other.
  const calls = [...Array(WARMUP + runs).keys()].flatMap((round) =>
    sides.map(([sb, , samples]) => ({ round, sb, samples })),
  );
  await calls.reduce<Promise<void>>(async (previous, { round, sb, samples }) => {
    await previous;
    const result = await call(sb);
    if (round >= WARMUP) samples.push(result.durationMs);
  }, Promise.resolve());
  return { name, bash: latencyOf(sides[0]?.[2] ?? []), ts: latencyOf(sides[1]?.[2] ?? []) };
}

const ms = (value: number): string => value.toFixed(1);

function table(rows: readonly Row[]): string {
  const body = rows.map(
    (r) => `| ${r.name} | ${ms(r.bash.p50)} | ${ms(r.ts.p50)} | ${ms(r.ts.p50 - r.bash.p50)} |`,
  );
  const head = ["| Fixture | bash module p50 | TS module p50 | TS − bash |", "|---|---|---|---|"];
  return `${[...head, ...body].join("\n")}\n`;
}

async function main(argv: readonly string[]): Promise<number> {
  const at = argv.indexOf("--runs");
  const runs = at === -1 ? 25 : Number(argv[at + 1]);
  if (!Number.isInteger(runs) || runs < 1) throw new Error("--runs takes a positive integer");
  const dir = mkdtempSync(join(tmpdir(), "ast-grep-latency-"));
  try {
    const pluginRoot = extractBase(BASH_BASE, dir);
    const slice: [string, Call][] = [
      ...NUDGE_SLICE.map((name): [string, Call] => [`pre: ${name}`, nudgeCall(name)]),
      ...SAVINGS_SLICE.map((name): [string, Call] => [`post: ${name}`, savingsCall(name)]),
    ];
    const rows = await slice.reduce<Promise<Row[]>>(
      async (done, [name, call]) => [...(await done), await row(name, call, runs, pluginRoot)],
      Promise.resolve([]),
    );
    process.stdout.write(table(rows));
    const over = rows.filter((r) => r.ts.p50 > r.bash.p50 + BUDGET_MS);
    for (const r of over) {
      process.stderr.write(
        `ast-grep-latency: ${r.name}: TS p50 ${ms(r.ts.p50)} ms > bash ${ms(r.bash.p50)} ms + ${String(BUDGET_MS)} ms\n`,
      );
    }
    return argv.includes("--assert") && over.length > 0 ? 1 : 0;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

process.exitCode = await main(process.argv.slice(2));
