/**
 * PostToolUse hook latency (#259, AC-7; epic #247 budget: p50 no worse than
 * the bash baseline + 5 ms on the same machine). Over a slice of the parity
 * corpus, spawned the way Claude Code spawns hooks, it measures
 * `bash post-tools/mod.sh` (the pre-#259 hooks.json command) against the
 * committed `hooks/dist/post-tools.js` behind its launcher. Post-tool modules
 * write state, so each variant runs in its own identically prepared sandbox
 * and reaches the same steady state during warm-up. The two alternate run by
 * run, so a change in machine load while a fixture is measured hits both.
 *
 * Usage: bun run tooling/src/benchmarks/post-tools-latency.ts [--runs N] [--assert]
 */
import { runPostBundle, runPostModSh } from "@toolu/conformance/harness/posttool";
import {
  POSTTOOL_CORPUS,
  postStdin,
  preparePost,
} from "@toolu/conformance/harness/posttool-corpus";
import { pretoolEnv } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { latencyOf, type Latency } from "@toolu/conformance/harness/timing";

const BUDGET_MS = 5;
const SLICE = [
  "gate-status: failing quality command advises",
  "gate-status: first passing quality command",
  "plain command is silent",
  "push-waiver: a successful push promotes the waiver",
  "ts-quality registry: console.log in a written file",
  "multi-path patch through ts-quality and rust-quality",
];

type Runner = typeof runPostModSh;
type Row = { name: string; bash: Latency; bundle: Latency };

const WARMUP = 2;

async function row(name: string, runs: number): Promise<Row> {
  const c = POSTTOOL_CORPUS.find((entry) => entry.name === name);
  if (c === undefined) throw new Error(`no corpus case named ${name}`);
  using bashBox = createSandbox({ git: true });
  using bundleBox = createSandbox({ git: true });
  const sides: [Runner, Sandbox, number[]][] = [
    [runPostModSh, bashBox, []],
    [runPostBundle, bundleBox, []],
  ];
  await Promise.all(sides.map(([, sb]) => preparePost(sb, "claude", c)));
  const calls = sides.map(([runner, sb]) => {
    const call = {
      cwd: sb.project,
      env: pretoolEnv(sb, "claude"),
      stdin: postStdin(sb, "claude", c),
    };
    return () => runner(sb, call);
  });
  // Sequential by design: concurrent spawns would contend and skew each other.
  const steps = [...Array(WARMUP + runs).keys()].flatMap((round) =>
    calls.map((call, at) => ({ round, at, call })),
  );
  await steps.reduce<Promise<void>>(async (previous, { round, at, call }) => {
    await previous;
    const result = await call();
    if (round >= WARMUP) sides[at]?.[2].push(result.durationMs);
  }, Promise.resolve());
  return { name, bash: latencyOf(sides[0]?.[2] ?? []), bundle: latencyOf(sides[1]?.[2] ?? []) };
}

const ms = (value: number): string => value.toFixed(1);

function table(rows: readonly Row[]): string {
  const body = rows.map(
    (r) =>
      `| ${r.name} | ${ms(r.bash.p50)} | ${ms(r.bundle.p50)} | ${ms(r.bundle.p50 - r.bash.p50)} |`,
  );
  return `${["| Fixture | bash p50 | bundle p50 | bundle − bash |", "|---|---|---|---|", ...body].join("\n")}\n`;
}

async function main(argv: readonly string[]): Promise<number> {
  const at = argv.indexOf("--runs");
  const runs = at === -1 ? 15 : Number(argv[at + 1]);
  if (!Number.isInteger(runs) || runs < 1) throw new Error("--runs takes a positive integer");
  const rows = await SLICE.reduce<Promise<Row[]>>(
    async (done, name) => [...(await done), await row(name, runs)],
    Promise.resolve([]),
  );
  process.stdout.write(table(rows));
  const over = rows.filter((r) => r.bundle.p50 > r.bash.p50 + BUDGET_MS);
  for (const r of over) {
    process.stderr.write(
      `post-tools-latency: ${r.name}: bundle p50 ${ms(r.bundle.p50)} ms > bash ${ms(r.bash.p50)} ms + ${String(BUDGET_MS)} ms\n`,
    );
  }
  return argv.includes("--assert") && over.length > 0 ? 1 : 0;
}

process.exitCode = await main(process.argv.slice(2));
