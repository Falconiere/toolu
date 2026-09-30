/**
 * Language-quality PostToolUse latency (epic #247 budget: p50 no worse than
 * the bash baseline + 5 ms on the same machine). For a slice of a plugin's
 * golden cases it times the committed post-tools bundle dispatching the
 * pre-port bash module (extracted from git at the golden's base commit)
 * against the same bundle running the TypeScript module in process. Each side
 * has its own identically prepared sandbox; runs alternate, so a load change
 * hits both, and warm-up runs bring both gate files to the same steady state.
 */
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { PretoolHost } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import type { RunResult } from "@toolu/conformance/harness/spawn";
import {
  latencyComparisonTable,
  measureAlternatingLatency,
  type Latency,
} from "@toolu/conformance/harness/timing";

type Registration =
  | { readonly kind: "bash"; readonly register: string }
  | { readonly kind: "bundle" };

/** The parts of one plugin's golden harness its latency benchmark uses. */
type QualityLatency<S, C extends { readonly name: string; readonly steps: readonly S[] }> = {
  /** The benchmark's name in messages and its temp directory. */
  readonly label: string;
  readonly cases: readonly C[];
  readonly slice: readonly string[];
  readonly base: string;
  extractBaseRegister(base: string, dir: string): string;
  setupCase(sb: Sandbox, host: PretoolHost, c: C, reg: Registration): Promise<unknown>;
  dispatchStep(sb: Sandbox, host: PretoolHost, c: C, step: S): Promise<RunResult>;
};

const BUDGET_MS = 5;
const WARMUP = 2;

type Row = { name: string; bash: Latency; ts: Latency };

async function row<S, C extends { readonly name: string; readonly steps: readonly S[] }>(
  h: QualityLatency<S, C>,
  name: string,
  runs: number,
  register: string,
): Promise<Row> {
  const c = h.cases.find((entry) => entry.name === name);
  const step = c?.steps.at(-1);
  if (c === undefined || step === undefined) throw new Error(`no golden case named ${name}`);
  using bashBox = createSandbox({ git: true });
  using tsBox = createSandbox({ git: true });
  const sides: [Sandbox, Registration][] = [
    [bashBox, { kind: "bash", register }],
    [tsBox, { kind: "bundle" }],
  ];
  await Promise.all(sides.map(([sb, reg]) => h.setupCase(sb, "claude", c, reg)));
  const [bash, ts] = await measureAlternatingLatency(
    [bashBox, tsBox],
    (sb) => h.dispatchStep(sb, "claude", c, step),
    { runs, warmup: WARMUP },
  );
  return { name, bash, ts };
}

const ms = (value: number): string => value.toFixed(1);

/** Print the comparison table; with `--assert`, 1 when a fixture exceeds the budget. */
export async function qualityLatency<
  S,
  C extends { readonly name: string; readonly steps: readonly S[] },
>(h: QualityLatency<S, C>, argv: readonly string[]): Promise<number> {
  const at = argv.indexOf("--runs");
  const runs = at === -1 ? 15 : Number(argv[at + 1]);
  if (!Number.isInteger(runs) || runs < 1) throw new Error("--runs takes a positive integer");
  const dir = mkdtempSync(join(tmpdir(), `${h.label}-`));
  try {
    const register = h.extractBaseRegister(h.base, dir);
    const rows = await h.slice.reduce<Promise<Row[]>>(
      async (done, name) => [...(await done), await row(h, name, runs, register)],
      Promise.resolve([]),
    );
    process.stdout.write(latencyComparisonTable(rows));
    const over = rows.filter((r) => r.ts.p50 > r.bash.p50 + BUDGET_MS);
    for (const r of over) {
      process.stderr.write(
        `${h.label}: ${r.name}: TS p50 ${ms(r.ts.p50)} ms > bash ${ms(r.bash.p50)} ms + ${String(BUDGET_MS)} ms\n`,
      );
    }
    return argv.includes("--assert") && over.length > 0 ? 1 : 0;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
