/**
 * Budget evidence for `@toolu/core/shell` (#284). Two probe hook entries are
 * built by the #249 pipeline (`stageBundles`): one that does nothing and one
 * that analyzes a command. The script reports:
 *
 * - bundle size: the shell probe adds at most 200,000 bytes unminified;
 * - cold start: interleaved spawns of both bundles from a temp dir with no
 *   node_modules; p50 delta at most 5 ms;
 * - parse and walk: `analyzeShell` plus the git and write helpers over every
 *   fixture command; p99 at most 0.1 ms.
 *
 * Numbers are machine-bound, so CI asserts only the size (bench-shell.test.ts).
 * `--assert` exits 1 when any budget is exceeded on this machine.
 *
 * Usage: bun run tooling/src/bench-shell.ts [--runs N] [--rounds N] [--json] [--assert]
 */
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { arch, cpus, platform, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { analyzeShell, pushTargets, runsGitSubcommand, writeTargets } from "@toolu/core/shell";
import { percentile } from "@toolu/conformance/harness/timing";
import { z } from "zod";
import { stageBundles } from "./build-plugins.ts";

const ROOT = resolve(import.meta.dir, "../..");
const SHELL_ENTRY = join(ROOT, "packages/toolu-core/src/shell/shell.ts");
export const PROBE_COMMAND = "git push origin HEAD:feat/x";
export const BUDGET = { bundleBytes: 200_000, coldStartMs: 5, parseP99Us: 100 };

const EMPTY_PROBE = 'process.stdout.write("{}\\n");\n';
const SHELL_PROBE = `import { analyzeShell, pushTargets, runsGitSubcommand } from ${JSON.stringify(SHELL_ENTRY)};
const analysis = analyzeShell(process.argv[2] ?? "");
const destination = pushTargets(analysis)[0]?.destination ?? null;
process.stdout.write(JSON.stringify({ push: runsGitSubcommand(analysis, "push"), destination }) + "\\n");
`;

export interface ShellBench {
  readonly machine: { bun: string; platform: string; arch: string; cpu: string; date: string };
  readonly bundle: { emptyBytes: number; shellBytes: number; deltaBytes: number };
  readonly coldStart: {
    runs: number;
    emptyP50: number;
    shellP50: number;
    deltaP50: number;
    emptyP90: number;
    shellP90: number;
  };
  readonly parse: {
    commands: number;
    samples: number;
    p50Us: number;
    p99Us: number;
    maxUs: number;
  };
  readonly probeOutput: string;
}

/** Build both probes with the plugin bundle pipeline; returns their paths in `out`. */
function buildProbes(work: string): { empty: string; shell: string } {
  const src = join(work, "tree/plugins/probe/hooks/src");
  mkdirSync(src, { recursive: true });
  writeFileSync(join(src, "empty.ts"), EMPTY_PROBE);
  writeFileSync(join(src, "shell.ts"), SHELL_PROBE);
  const out = join(work, "out");
  stageBundles(join(work, "tree"), out);
  return { empty: join(out, "probe/empty.js"), shell: join(out, "probe/shell.js") };
}

function spawnMs(bundle: string, cwd: string): number {
  const started = performance.now();
  const res = spawnSync(process.execPath, [bundle, PROBE_COMMAND], { cwd, encoding: "utf8" });
  if (res.status !== 0) throw new Error(`${bundle} exited ${String(res.status)}: ${res.stderr}`);
  return performance.now() - started;
}

/** Interleaved sequential spawns, so machine load hits both bundles alike. */
function coldStart(
  bundles: { empty: string; shell: string },
  cwd: string,
  runs: number,
): ShellBench["coldStart"] {
  const empty: number[] = [];
  const shell: number[] = [];
  for (let i = 0; i < 5; i++) spawnMs(i % 2 === 0 ? bundles.empty : bundles.shell, cwd);
  for (let i = 0; i < runs; i++) {
    if (i % 2 === 0) {
      empty.push(spawnMs(bundles.empty, cwd));
      shell.push(spawnMs(bundles.shell, cwd));
    } else {
      shell.push(spawnMs(bundles.shell, cwd));
      empty.push(spawnMs(bundles.empty, cwd));
    }
  }
  const emptyP50 = percentile(empty, 50);
  const shellP50 = percentile(shell, 50);
  return {
    runs,
    emptyP50,
    shellP50,
    deltaP50: shellP50 - emptyP50,
    emptyP90: percentile(empty, 90),
    shellP90: percentile(shell, 90),
  };
}

const Fixture = z.object({ cases: z.array(z.object({ command: z.string() })) });

/** Every command in the checked-in shell fixtures. */
export function fixtureCommands(): string[] {
  return ["bats-parity.json", "issue-283.json"].flatMap((name) => {
    const text = readFileSync(join(ROOT, "tooling/fixtures/shell", name), "utf8");
    const commands = Fixture.parse(JSON.parse(text)).cases.map((c) => c.command);
    return commands.map((command) =>
      command.replaceAll("$TMP", "/tmp/t").replaceAll("$MKTEMP", "/tmp/m"),
    );
  });
}

function analyzeOnce(command: string): number {
  const started = performance.now();
  const analysis = analyzeShell(command);
  runsGitSubcommand(analysis, "push");
  pushTargets(analysis);
  writeTargets(analysis);
  return (performance.now() - started) * 1000;
}

function parseTimes(rounds: number): ShellBench["parse"] {
  const commands = fixtureCommands();
  for (const command of commands) for (let i = 0; i < 3; i++) analyzeOnce(command);
  const samples = Array.from({ length: rounds }, () => commands.map(analyzeOnce)).flat();
  return {
    commands: commands.length,
    samples: samples.length,
    p50Us: percentile(samples, 50),
    p99Us: percentile(samples, 99),
    maxUs: Math.max(...samples),
  };
}

export function benchShell(options: { runs: number; rounds: number }): ShellBench {
  // Staged inside the repository (ignored node_modules/.cache) so the bundle's
  // module-path comments are repo-relative, as in a committed plugin bundle.
  const cache = join(ROOT, "node_modules/.cache");
  mkdirSync(cache, { recursive: true });
  const work = mkdtempSync(join(cache, "bench-shell-"));
  const runDir = mkdtempSync(join(tmpdir(), "bench-shell-run-"));
  try {
    const bundles = buildProbes(work);
    const emptyBytes = statSync(bundles.empty).size;
    const shellBytes = statSync(bundles.shell).size;
    // Run copies from outside the repository: no node_modules anywhere above them.
    const standalone = { empty: join(runDir, "empty.js"), shell: join(runDir, "shell.js") };
    copyFileSync(bundles.empty, standalone.empty);
    copyFileSync(bundles.shell, standalone.shell);
    const probe = spawnSync(process.execPath, [standalone.shell, PROBE_COMMAND], {
      cwd: runDir,
      encoding: "utf8",
    });
    return {
      machine: {
        bun: Bun.version,
        platform: platform(),
        arch: arch(),
        cpu: cpus()[0]?.model ?? "unknown",
        date: new Date().toISOString().slice(0, 10),
      },
      bundle: { emptyBytes, shellBytes, deltaBytes: shellBytes - emptyBytes },
      coldStart: coldStart(standalone, runDir, options.runs),
      parse: parseTimes(options.rounds),
      probeOutput: probe.stdout.trim(),
    };
  } finally {
    rmSync(work, { recursive: true, force: true });
    rmSync(runDir, { recursive: true, force: true });
  }
}

/** Budget violations, empty when every budget holds. */
export function overBudget(bench: ShellBench): string[] {
  const problems: string[] = [];
  if (bench.bundle.deltaBytes > BUDGET.bundleBytes)
    problems.push(`bundle +${bench.bundle.deltaBytes} B > ${BUDGET.bundleBytes} B`);
  if (bench.coldStart.deltaP50 > BUDGET.coldStartMs)
    problems.push(
      `cold start +${bench.coldStart.deltaP50.toFixed(2)} ms > ${BUDGET.coldStartMs} ms`,
    );
  if (bench.parse.p99Us > BUDGET.parseP99Us)
    problems.push(`parse p99 ${bench.parse.p99Us.toFixed(1)} µs > ${BUDGET.parseP99Us} µs`);
  return problems;
}

const ms = (value: number) => `${value.toFixed(2)} ms`;

function report(bench: ShellBench): string {
  const { machine: m, bundle: b, coldStart: c, parse: p } = bench;
  return [
    `bench:shell — bun ${m.bun}, ${m.platform} ${m.arch}, ${m.cpu}, ${m.date}`,
    `bundle      empty ${b.emptyBytes} B, shell ${b.shellBytes} B, delta ${b.deltaBytes} B (budget ${BUDGET.bundleBytes} B)`,
    `cold start  ${c.runs} interleaved runs: empty p50 ${ms(c.emptyP50)} p90 ${ms(c.emptyP90)}, shell p50 ${ms(c.shellP50)} p90 ${ms(c.shellP90)}, delta p50 ${ms(c.deltaP50)} (budget ${BUDGET.coldStartMs} ms)`,
    `parse+walk  ${p.commands} commands, ${p.samples} samples: p50 ${p.p50Us.toFixed(1)} µs, p99 ${p.p99Us.toFixed(1)} µs, max ${p.maxUs.toFixed(1)} µs (budget p99 ${BUDGET.parseP99Us} µs)`,
    `probe       ${bench.probeOutput}`,
  ].join("\n");
}

function numberFlag(args: readonly string[], name: string, fallback: number): number {
  const index = args.indexOf(name);
  const value = index === -1 ? fallback : Number(args[index + 1]);
  if (!Number.isInteger(value) || value < 1) throw new Error(`${name} needs a positive integer`);
  return value;
}

function runBench(args: readonly string[]): number {
  const known = new Set(["--runs", "--rounds", "--json", "--assert"]);
  const unknown = args.filter(
    (arg, i) => arg.startsWith("--") && !known.has(arg) && !known.has(args[i - 1] ?? ""),
  );
  if (unknown.length > 0) throw new Error(`unknown option ${unknown.join(" ")}`);
  const bench = benchShell({
    runs: numberFlag(args, "--runs", 40),
    rounds: numberFlag(args, "--rounds", 20),
  });
  process.stdout.write(`${args.includes("--json") ? JSON.stringify(bench) : report(bench)}\n`);
  const problems = overBudget(bench);
  for (const problem of problems) process.stderr.write(`OVER BUDGET  ${problem}\n`);
  return args.includes("--assert") && problems.length > 0 ? 1 : 0;
}

/** CLI entry: a bad argument or a failed build is reported on stderr with exit 1. */
export function main(args: readonly string[]): number {
  try {
    return runBench(args);
  } catch (error: unknown) {
    process.stderr.write(
      `bench:shell: ${error instanceof Error ? error.message : String(error)}\n`,
    );
    return 1;
  }
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));
