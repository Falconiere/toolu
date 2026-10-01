/**
 * PostToolUse hook latency (#259, AC-7; epic #247 budget: p50 no worse than
 * the bash baseline + 5 ms on the same machine). Over a slice of the parity
 * corpus, spawned the way Claude Code spawns hooks, it measures
 * `bash post-tools/mod.sh` extracted from v7.2.0 against the
 * committed `hooks/dist/post-tools.js` behind its launcher. Post-tool modules
 * write state, so each variant runs in its own identically prepared sandbox
 * and reaches the same steady state during warm-up. The two alternate run by
 * run, so a change in machine load while a fixture is measured hits both.
 *
 * End-to-end diagnostic; the incremental cold-start assertion lives in
 * `bun run bench:shell --assert`.
 * Usage: bun run tooling/src/benchmarks/post-tools-latency.ts [--runs N]
 */
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { arch, platform, tmpdir } from "node:os";
import { join } from "node:path";
import { runPostBundle } from "@toolu/conformance/harness/posttool";
import {
  POSTTOOL_CORPUS,
  postStdin,
  preparePost,
} from "@toolu/conformance/harness/posttool-corpus";
import { pretoolEnv, REPO_ROOT, type PretoolRun } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type RunResult } from "@toolu/conformance/harness/spawn";
import { latencyOf, type Latency } from "@toolu/conformance/harness/timing";

const BUDGET_MS = 5;
const SLICE = [
  "gate-status: failing quality command advises",
  "gate-status: first passing quality command",
  "plain command is silent",
  "push-waiver: a successful push promotes the waiver",
  "registry: a multi-path patch records a gate entry per path",
];

type Runner = (sb: Sandbox, call: PretoolRun) => Promise<RunResult>;
type Row = { name: string; bash: Latency; bundle: Latency };

const WARMUP = 2;

async function row(name: string, runs: number, baseline: string): Promise<Row> {
  const c = POSTTOOL_CORPUS.find((entry) => entry.name === name);
  if (c === undefined) throw new Error(`no corpus case named ${name}`);
  using bashBox = createSandbox({ git: true });
  using bundleBox = createSandbox({ git: true });
  const sides: [Runner, Sandbox, number[]][] = [
    [
      (sb, call) => {
        void sb;
        return run(["bash", baseline], call);
      },
      bashBox,
      [],
    ],
    [runPostBundle, bundleBox, []],
  ];
  for (const [, sb] of sides) preparePost(sb, "claude", c);
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
  const dir = mkdtempSync(join(tmpdir(), "toolu-posttool-bench-"));
  try {
    const extract = spawnSync(
      "sh",
      ["-c", 'git archive v7.2.0 plugins/toolu | tar -x -C "$1"', "sh", dir],
      { cwd: REPO_ROOT, encoding: "utf8" },
    );
    if (extract.status !== 0) throw new Error(`extracting v7.2.0 failed: ${extract.stderr.trim()}`);
    const baseline = join(dir, "plugins/toolu/hooks/post-tools/mod.sh");
    const rows = await SLICE.reduce<Promise<Row[]>>(
      async (done, name) => [...(await done), await row(name, runs, baseline)],
      Promise.resolve([]),
    );
    process.stdout.write(
      `Platform: ${platform()} ${arch()}; Bun ${Bun.version}; Bash baseline v7.2.0\n` +
        "End-to-end hook delta: diagnostic; incremental cold-start gate: bun run bench:shell --assert\n\n" +
        table(rows),
    );
    const over = rows.filter((r) => r.bundle.p50 > r.bash.p50 + BUDGET_MS);
    for (const r of over) {
      process.stderr.write(
        `post-tools-latency: ${r.name}: bundle p50 ${ms(r.bundle.p50)} ms > bash ${ms(r.bash.p50)} ms + ${String(BUDGET_MS)} ms\n`,
      );
    }
    return 0;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

process.exitCode = await main(process.argv.slice(2));
