/**
 * ts-quality PostToolUse latency (#265, AC-8; epic #247 budget: p50 no worse
 * than the bash baseline + 5 ms on the same machine). For a slice of the
 * golden cases it times the committed post-tools bundle dispatching the
 * pre-port bash module (extracted from git at the golden's base commit)
 * against the same bundle running the TypeScript module in process. Each side
 * has its own identically prepared sandbox; runs alternate, so a load change
 * hits both, and warm-up runs bring both gate files to the same steady state.
 *
 * Usage: bun run tooling/src/benchmarks/ts-quality-latency.ts [--runs N] [--assert]
 */
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import {
  latencyComparisonTable,
  measureAlternatingLatency,
  type Latency,
} from "@toolu/conformance/harness/timing";
import { TS_CASES } from "../../../plugins/ts-quality/hooks/src/__tests__/cases.ts";
import {
  BASH_BASE,
  dispatchStep,
  extractBaseRegister,
  setupCase,
  type Registration,
} from "../../../plugins/ts-quality/hooks/src/__tests__/golden-harness.ts";

const BUDGET_MS = 5;
const WARMUP = 2;
const SLICE = [
  "assembled: three violations in fragment order",
  "errors: empty catch block",
  "no-mocks: vi.mock",
  "size: long class method",
  "assembled: clean file",
];

type Row = { name: string; bash: Latency; ts: Latency };

async function row(name: string, runs: number, register: string): Promise<Row> {
  const c = TS_CASES.find((entry) => entry.name === name);
  const step = c?.steps.at(-1);
  if (c === undefined || step === undefined) throw new Error(`no golden case named ${name}`);
  using bashBox = createSandbox({ git: true });
  using tsBox = createSandbox({ git: true });
  const sides: [Sandbox, Registration][] = [
    [bashBox, { kind: "bash", register }],
    [tsBox, { kind: "bundle" }],
  ];
  await Promise.all(sides.map(([sb, reg]) => setupCase(sb, "claude", c, reg)));
  const [bash, ts] = await measureAlternatingLatency(
    [bashBox, tsBox],
    (sb) => dispatchStep(sb, "claude", c, step),
    { runs, warmup: WARMUP },
  );
  return { name, bash, ts };
}

const ms = (value: number): string => value.toFixed(1);

async function main(argv: readonly string[]): Promise<number> {
  const at = argv.indexOf("--runs");
  const runs = at === -1 ? 15 : Number(argv[at + 1]);
  if (!Number.isInteger(runs) || runs < 1) throw new Error("--runs takes a positive integer");
  const dir = mkdtempSync(join(tmpdir(), "ts-quality-latency-"));
  try {
    const register = extractBaseRegister(BASH_BASE, dir);
    const rows = await SLICE.reduce<Promise<Row[]>>(
      async (done, name) => [...(await done), await row(name, runs, register)],
      Promise.resolve([]),
    );
    process.stdout.write(latencyComparisonTable(rows));
    const over = rows.filter((r) => r.ts.p50 > r.bash.p50 + BUDGET_MS);
    for (const r of over) {
      process.stderr.write(
        `ts-quality-latency: ${r.name}: TS p50 ${ms(r.ts.p50)} ms > bash ${ms(r.bash.p50)} ms + ${String(BUDGET_MS)} ms\n`,
      );
    }
    return argv.includes("--assert") && over.length > 0 ? 1 : 0;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

process.exitCode = await main(process.argv.slice(2));
