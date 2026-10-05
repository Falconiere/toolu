/**
 * Budget evidence for `@toolu/core/shell` (#284). Probe hook entries are built
 * by the #249 pipeline (`stageBundles`):
 *
 * - `empty`: does nothing, the baseline;
 * - `shell`: imports and calls every runtime export of `@toolu/core/shell`;
 * - `writes`: `analyzeShell` plus `writeTargets` from `@toolu/core/shell/writes`,
 *   the entry protected-files needs;
 * - `together`: every public export of both entries in one bundle.
 *
 * Budgets: `shell` and `writes` each add at most 200,000 bytes, and
 * `together`, the PreToolUse dispatcher's case (#258), at most 205,000 bytes by
 * product-owner decision (unbash alone is about 175 KB). All probes use the
 * readable unminified bundles shipped by #249. It runs `together` against `empty`, interleaved from a temp
 * dir with no node_modules: p50 delta at most 5 ms. Parse and walk time
 * `analyzeShell` plus the git and write helpers over every fixture command:
 * p99 at most 0.1 ms.
 *
 * Numbers are machine-bound. `--assert` always enforces bundle and parse
 * budgets. The 5 ms cold-start budget is hard on macOS Apple Silicon (or with
 * TOOLU_LATENCY_ENFORCE=1); other machines measure and report it without
 * failing the gate. GitHub Actions also receives a job-summary table.
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
import { analyzeShell, pushTargets, runsGitSubcommand } from "@toolu/core/shell";
import { writeTargets } from "@toolu/core/shell/writes";
import { percentile } from "@toolu/conformance/harness/timing";
import { z } from "zod";
import { stageBundles } from "./build-plugins.ts";
import { appendJobSummary } from "./bench-shell-summary.ts";

const ROOT = resolve(import.meta.dir, "../..");
const SHELL_ENTRY = JSON.stringify(join(ROOT, "packages/toolu-core/src/shell/shell.ts"));
const WRITES_ENTRY = JSON.stringify(join(ROOT, "packages/toolu-core/src/shell/shell-writes.ts"));
export const PROBE_COMMAND = "git push origin HEAD:feat/x";
export const BUDGET = {
  bundleBytes: 200_000,
  writesBundleBytes: 200_000,
  togetherBundleBytes: 205_000,
  coldStartMs: 5,
  parseP99Us: 100,
};

const SHELL_IMPORT = `import {
  MAX_RUN_DEPTH, MAX_SHELL_INPUT, analyzeShell, commitMessages, gitInvocation, matchesRule,
  pushTargets, runsGitSubcommand, shellAnalysisOf,
} from ${SHELL_ENTRY};
`;
const WRITES_IMPORT = `import { writeTargets } from ${WRITES_ENTRY};\n`;
const SHELL_BODY = `const event = { type: "shell/pre", sessionId: "s", cwd: "/", projectRoot: "/", worktree: "/",
  toolCallId: "c", toolName: "Bash", toolInput: {}, command: process.argv[2] ?? "" };
const analysis = shellAnalysisOf(event);
const commit = analysis.commands.map(gitInvocation).find((git) => git?.subcommand === "commit");
const report = {
  push: runsGitSubcommand(analysis, "push"),
  destination: pushTargets(analysis)[0]?.destination ?? null,
  commands: analyzeShell(analysis.source).commands.length,
  messages: commit === undefined ? [] : commitMessages(commit),
  rule: analysis.commands.some((c) => matchesRule(c, "node -e")),
  limits: [MAX_RUN_DEPTH, MAX_SHELL_INPUT],
};
`;
const PROBES = {
  empty: 'process.stdout.write("{}\\n");\n',
  shell: `${SHELL_IMPORT}${SHELL_BODY}process.stdout.write(JSON.stringify(report) + "\\n");\n`,
  writes: `import { analyzeShell } from ${SHELL_ENTRY};\n${WRITES_IMPORT}const writes = writeTargets(analyzeShell(process.argv[2] ?? ""));\nprocess.stdout.write(JSON.stringify({ writes: writes.length }) + "\\n");\n`,
  together: `${SHELL_IMPORT}${WRITES_IMPORT}${SHELL_BODY}process.stdout.write(JSON.stringify({ ...report, writes: writeTargets(analysis).length }) + "\\n");\n`,
};

type Probe = keyof typeof PROBES;

export interface ShellBench {
  readonly machine: { bun: string; platform: string; arch: string; cpu: string; date: string };
  readonly bundle: {
    emptyBytes: number;
    /** `@toolu/core/shell`, every runtime export. */
    deltaBytes: number;
    /** `analyzeShell` plus `@toolu/core/shell/writes`. */
    writesDeltaBytes: number;
    /** Both entries together, every public export of each. */
    togetherDeltaBytes: number;
  };
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

/** Build the probes with the plugin bundle pipeline; returns each bundle's path. */
function buildProbes(work: string): Record<Probe, string> {
  const src = join(work, "tree/plugins/probe/hooks/src");
  mkdirSync(src, { recursive: true });
  for (const [name, source] of Object.entries(PROBES))
    writeFileSync(join(src, `${name}.ts`), source);
  const out = join(work, "out/probe");
  stageBundles(join(work, "tree"), join(work, "out"));
  return {
    empty: join(out, "empty.js"),
    shell: join(out, "shell.js"),
    writes: join(out, "writes.js"),
    together: join(out, "together.js"),
  };
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
    const text = readFileSync(join(ROOT, "fixtures/shell", name), "utf8");
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
  // module-path comments are repo-relative, like those of a committed plugin bundle.
  const cache = join(ROOT, "node_modules/.cache");
  mkdirSync(cache, { recursive: true });
  const work = mkdtempSync(join(cache, "bench-shell-"));
  const runDir = mkdtempSync(join(tmpdir(), "bench-shell-run-"));
  try {
    const bundles = buildProbes(work);
    const size = (probe: Probe) => statSync(bundles[probe]).size;
    const emptyBytes = size("empty");
    // Run copies from outside the repository: no node_modules anywhere above them.
    // Cold start times the `together` probe, the heaviest.
    const standalone = { empty: join(runDir, "empty.js"), shell: join(runDir, "together.js") };
    copyFileSync(bundles.empty, standalone.empty);
    copyFileSync(bundles.together, standalone.shell);
    copyFileSync(bundles.shell, join(runDir, "shell.js"));
    const probe = spawnSync(process.execPath, [join(runDir, "shell.js"), PROBE_COMMAND], {
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
      bundle: {
        emptyBytes,
        deltaBytes: size("shell") - emptyBytes,
        writesDeltaBytes: size("writes") - emptyBytes,
        togetherDeltaBytes: size("together") - emptyBytes,
      },
      coldStart: coldStart(standalone, runDir, options.runs),
      parse: parseTimes(options.rounds),
      probeOutput: probe.stdout.trim(),
    };
  } finally {
    rmSync(work, { recursive: true, force: true });
    rmSync(runDir, { recursive: true, force: true });
  }
}

/** Owner's macOS Apple Silicon class is the cold-start acceptance machine. */
export function coldStartIsHard(
  machine: Pick<ShellBench["machine"], "platform" | "arch">,
  override: string | undefined,
): boolean {
  return (machine.platform === "darwin" && machine.arch === "arm64") || override === "1";
}

/** Hard budget violations; size and parse are hard on every platform. */
export function overBudget(bench: ShellBench, hardColdStart = true): string[] {
  const problems: string[] = [];
  if (bench.bundle.deltaBytes > BUDGET.bundleBytes)
    problems.push(`bundle +${bench.bundle.deltaBytes} B > ${BUDGET.bundleBytes} B`);
  if (bench.bundle.writesDeltaBytes > BUDGET.writesBundleBytes)
    problems.push(
      `writes bundle +${bench.bundle.writesDeltaBytes} B > ${BUDGET.writesBundleBytes} B`,
    );
  if (bench.bundle.togetherDeltaBytes > BUDGET.togetherBundleBytes)
    problems.push(
      `together bundle +${bench.bundle.togetherDeltaBytes} B > ${BUDGET.togetherBundleBytes} B`,
    );
  if (hardColdStart && bench.coldStart.deltaP50 > BUDGET.coldStartMs)
    problems.push(
      `cold start +${bench.coldStart.deltaP50.toFixed(2)} ms > ${BUDGET.coldStartMs} ms`,
    );
  if (bench.parse.p99Us > BUDGET.parseP99Us)
    problems.push(`parse p99 ${bench.parse.p99Us.toFixed(1)} µs > ${BUDGET.parseP99Us} µs`);
  return problems;
}

const ms = (value: number) => `${value.toFixed(2)} ms`;

function report(bench: ShellBench, hardColdStart: boolean): string {
  const { machine: m, bundle: b, coldStart: c, parse: p } = bench;
  return [
    `bench:shell — bun ${m.bun}, ${m.platform} ${m.arch}, ${m.cpu}, ${m.date}`,
    `bundle      shipped unminified: empty ${b.emptyBytes} B; @toolu/core/shell +${b.deltaBytes} B (budget ${BUDGET.bundleBytes} B); analyzeShell + writes +${b.writesDeltaBytes} B (budget ${BUDGET.writesBundleBytes} B); both entries +${b.togetherDeltaBytes} B (budget ${BUDGET.togetherBundleBytes} B)`,
    `cold start  shipped unminified, ${c.runs} interleaved runs: empty p50 ${ms(c.emptyP50)} p90 ${ms(c.emptyP90)}, both-entries p50 ${ms(c.shellP50)} p90 ${ms(c.shellP90)}, delta p50 ${ms(c.deltaP50)} (budget ${BUDGET.coldStartMs} ms; ${hardColdStart ? "hard assertion" : "report-only on this machine"})`,
    `parse+walk  ${p.commands} commands, ${p.samples} samples: p50 ${p.p50Us.toFixed(1)} µs, p99 ${p.p99Us.toFixed(1)} µs, max ${p.maxUs.toFixed(1)} µs (budget p99 ${BUDGET.parseP99Us} µs)`,
    `probe       ${bench.probeOutput}`,
  ].join("\n");
}

/** A value-taking flag's positive integer; the next word must not be missing or another flag. */
function positive(name: string, raw: string | undefined): number {
  if (raw === undefined || raw.startsWith("--")) throw new Error(`${name} needs a value`);
  const value = Number(raw);
  if (!Number.isInteger(value) || value < 1) throw new Error(`${name} needs a positive integer`);
  return value;
}

interface Cli {
  runs: number;
  rounds: number;
  json: boolean;
  assert: boolean;
}

/** Read the arguments in order, so a value is never mistaken for an option or the reverse. */
function parseCli(args: readonly string[]): Cli {
  const cli: Cli = { runs: 40, rounds: 20, json: false, assert: false };
  for (let i = 0; i < args.length; i++) {
    const arg = args[i] ?? "";
    if (arg === "--json") cli.json = true;
    else if (arg === "--assert") cli.assert = true;
    else if (arg === "--runs") cli.runs = positive(arg, args[++i]);
    else if (arg === "--rounds") cli.rounds = positive(arg, args[++i]);
    else throw new Error(`unknown option ${arg}`);
  }
  return cli;
}

function runBench(args: readonly string[]): number {
  const cli = parseCli(args);
  const bench = benchShell({ runs: cli.runs, rounds: cli.rounds });
  const hardColdStart = coldStartIsHard(bench.machine, process.env.TOOLU_LATENCY_ENFORCE);
  process.stdout.write(`${cli.json ? JSON.stringify(bench) : report(bench, hardColdStart)}\n`);
  appendJobSummary(bench, BUDGET, hardColdStart);
  const problems = overBudget(bench, hardColdStart);
  for (const problem of problems) process.stderr.write(`OVER BUDGET  ${problem}\n`);
  return cli.assert && problems.length > 0 ? 1 : 0;
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
