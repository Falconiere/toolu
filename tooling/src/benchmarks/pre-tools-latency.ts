/**
 * PreToolUse hook latency (#258, AC-5; epic #247 budget: p50 no worse than the
 * bash baseline + 5 ms on the same machine). Over a representative slice of
 * the parity corpus, spawned the way Claude Code spawns hooks, measures in turn:
 *
 *   bash      `bash pre-tools/mod.sh`, the pre-#258 hooks.json command
 *   bundle    the committed `hooks/dist/pre-tools.js` behind its launcher,
 *             every module on its bash fallback
 *   native    the same dispatcher with one module's fallback off
 *             (`hooks/src/__tests__/fixtures/pre-tools-native.ts`)
 *
 * Usage: bun run tooling/src/benchmarks/pre-tools-latency.ts [--runs N] [--assert]
 */
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { toStdin } from "@toolu/conformance/harness/fixtures";
import {
  pretoolEnv,
  runBashEntry,
  runBundleEntry,
  TOOLU_PLUGIN,
  type PretoolRun,
} from "@toolu/conformance/harness/pretool";
import { PRETOOL_CORPUS, prepare } from "@toolu/conformance/harness/pretool-corpus";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run, type RunResult } from "@toolu/conformance/harness/spawn";
import { measureLatency, type Latency } from "@toolu/conformance/harness/timing";

const BUDGET_MS = 5;
const SLICE = [
  "protected-files: .env edit asks",
  "code-edit-rules: feature docs",
  "multi-file patch: advisories only",
  "plain command is silent",
  "commit-gate: commit advice",
  "push-review + docs-sync: unreviewed push advises",
  "ast-grep registry: structural Grep nudge",
];

type Variant = "bash" | "bundle" | "native";
type Row = { name: string } & Record<Variant, Latency>;

async function buildNative(outDir: string): Promise<string> {
  const source = join(TOOLU_PLUGIN, "hooks/src/__tests__/fixtures/pre-tools-native.ts");
  const result = await Bun.build({
    entrypoints: [source],
    target: "bun",
    format: "esm",
    sourcemap: "none",
    outdir: outDir,
    define: { HOOKS_DIR: JSON.stringify(join(TOOLU_PLUGIN, "hooks")) },
  });
  const [output] = result.outputs;
  if (!result.success || output === undefined) {
    throw new Error(`bundling ${source} failed:\n${result.logs.map(String).join("\n")}`);
  }
  return output.path;
}

function runners(native: string): Record<Variant, (call: PretoolRun) => Promise<RunResult>> {
  return {
    bash: (call) => runBashEntry("pre-tools", call),
    bundle: (call) => runBundleEntry("pre-tools", call),
    native: (call) => run([process.execPath, native], call),
  };
}

async function measureCase(name: string, native: string, runs: number): Promise<Row> {
  const c = PRETOOL_CORPUS.find((entry) => entry.name === name);
  if (c === undefined) throw new Error(`no corpus case named ${name}`);
  using sb = createSandbox({ git: true });
  const extra = await prepare(sb, "claude", c);
  const stdin = c.stdin ?? JSON.stringify(toStdin("claude", c.fixture(sb), { cwd: sb.project }));
  const call = { cwd: sb.project, env: pretoolEnv(sb, "claude", extra), stdin };
  const via = runners(native);
  const opts = { runs, warmup: 2 };
  // Sequential by design: concurrent spawns would contend and skew each other.
  const bash = await measureLatency(() => via.bash(call), opts);
  const bundle = await measureLatency(() => via.bundle(call), opts);
  const nativeLatency = await measureLatency(() => via.native(call), opts);
  return { name, bash, bundle, native: nativeLatency };
}

function ms(value: number): string {
  return value.toFixed(1);
}

function report(rows: readonly Row[]): string {
  const lines = [
    "| Fixture | bash p50 | bundle p50 | native p50 | bundle − bash | native − bash |",
    "|---|---|---|---|---|---|",
    ...rows.map(
      (r) =>
        `| ${r.name} | ${ms(r.bash.p50)} | ${ms(r.bundle.p50)} | ${ms(r.native.p50)} | ${ms(r.bundle.p50 - r.bash.p50)} | ${ms(r.native.p50 - r.bash.p50)} |`,
    ),
  ];
  return `${lines.join("\n")}\n`;
}

function overBudget(rows: readonly Row[]): string[] {
  return rows.flatMap((r) =>
    (["bundle", "native"] as const)
      .filter((v) => r[v].p50 > r.bash.p50 + BUDGET_MS)
      .map(
        (v) =>
          `${r.name}: ${v} p50 ${ms(r[v].p50)} ms > bash ${ms(r.bash.p50)} ms + ${String(BUDGET_MS)} ms`,
      ),
  );
}

async function main(argv: readonly string[]): Promise<number> {
  const runsAt = argv.indexOf("--runs");
  const runs = runsAt === -1 ? 15 : Number(argv[runsAt + 1]);
  const outDir = mkdtempSync(join(tmpdir(), "toolu-pretool-bench-"));
  try {
    const native = await buildNative(outDir);
    const rows = await SLICE.reduce<Promise<Row[]>>(
      async (done, name) => [...(await done), await measureCase(name, native, runs)],
      Promise.resolve([]),
    );
    process.stdout.write(report(rows));
    const over = overBudget(rows);
    for (const line of over) process.stderr.write(`pre-tools-latency: ${line}\n`);
    return argv.includes("--assert") && over.length > 0 ? 1 : 0;
  } finally {
    rmSync(outDir, { recursive: true, force: true });
  }
}

process.exitCode = await main(process.argv.slice(2));
