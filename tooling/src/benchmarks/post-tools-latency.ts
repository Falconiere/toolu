/**
 * PostToolUse hook latency (#259, AC-7; epic #247 budget: p50 no worse than
 * the bash baseline + 5 ms on the same machine). Over a slice of the parity
 * corpus, spawned the way Claude Code spawns hooks, it measures
 * `bash post-tools/mod.sh` (the pre-#259 hooks.json command) against the
 * committed `hooks/dist/post-tools.js` behind its launcher. Post-tool modules
 * write state, so each variant runs in its own identically prepared sandbox
 * and reaches the same steady state during warm-up.
 *
 * Usage: bun run tooling/src/benchmarks/post-tools-latency.ts [--runs N] [--assert]
 */
import { runPostBundle, runPostModSh } from "@toolu/conformance/harness/posttool";
import {
  POSTTOOL_CORPUS,
  postStdin,
  preparePost,
  type PosttoolCase,
} from "@toolu/conformance/harness/posttool-corpus";
import { pretoolEnv } from "@toolu/conformance/harness/pretool";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { measureLatency, type Latency } from "@toolu/conformance/harness/timing";

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

async function measure(c: PosttoolCase, runner: Runner, runs: number): Promise<Latency> {
  using sb = createSandbox({ git: true });
  await preparePost(sb, "claude", c);
  const call = {
    cwd: sb.project,
    env: pretoolEnv(sb, "claude"),
    stdin: postStdin(sb, "claude", c),
  };
  // Awaited here: `using` removes the sandbox when this function returns.
  return await measureLatency(() => runner(sb, call), { runs, warmup: 2 });
}

async function row(name: string, runs: number): Promise<Row> {
  const c = POSTTOOL_CORPUS.find((entry) => entry.name === name);
  if (c === undefined) throw new Error(`no corpus case named ${name}`);
  // Sequential by design: concurrent spawns would contend and skew each other.
  const bash = await measure(c, runPostModSh, runs);
  const bundle = await measure(c, runPostBundle, runs);
  return { name, bash, bundle };
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
