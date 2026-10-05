#!/usr/bin/env bun
/**
 * Hook resource benchmark (#410): `bun run bench:hooks [flags]` spawns every hook
 * entry in the repository with a fixed payload in a fixed sandbox, `--warmup`
 * times unmeasured then `--runs` times through `cargo xtask measure`, and reports
 * p50/p90 of max RSS, CPU (children included) and wall. Bun entries run their
 * `hooks.json` launcher under `/bin/sh -c`; an entry `TOOLU_IMPL` selects runs
 * `/bin/sh -c 'exec "$0" "$@"' toolu … hook <entry>`, the same shape, until #412.
 * `--assert` selects the entries in `fixtures/rust-ported.json` and fails when one
 * is over its `benchmarks/hook-budgets.json` p50 budget. Exit 0 ok, 1 over or
 * missing budget, 2 usage or setup error. See docs/resource-budgets.md.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { arch, cpus, platform } from "node:os";
import { dirname, join, resolve } from "node:path";
import { resolveEntryCommand } from "@toolu/conformance/harness/entry-command";
import { pretoolEnv } from "@toolu/conformance/harness/pretool";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { PortsError, portSelector, readPorted } from "../rust-conformance.ts";
import { headCommit, today } from "./lib/root.ts";
import { parseArgs, type BenchArgs } from "./lib/hook-args.ts";
import {
  BenchError,
  Budgets,
  type EntryResult,
  type HookResult,
  type MeasureReport,
  Payloads,
  loadJson,
} from "./lib/hook-data.ts";
import { discoverEntries, type HookEntry } from "./lib/hook-entries.ts";
import { benchSandbox, payloadStdin } from "./lib/hook-fixture.ts";
import { buildMeasurer, inSequence, measureOnce, type Spawn } from "./lib/hook-measurer.ts";
import { summarize, table, violations } from "./lib/hook-report.ts";

const ROOT = resolve(import.meta.dir, "../../..");
/** The issue's CI limit for one bench run's measured phase (AC-8). */
const MAX_ELAPSED_MS = 60_000;

type Env = Record<string, string | undefined>;

/** The entries to measure, after checking the payloads cover exactly the discovered set. */
function selected(entries: readonly HookEntry[], payloads: Payloads, args: BenchArgs): HookEntry[] {
  const ids = new Set(entries.map((e) => e.id));
  const missing = entries.filter((e) => payloads.entries[e.id] === undefined).map((e) => e.id);
  const stale = Object.keys(payloads.entries).filter((id) => !ids.has(id));
  const problems = [
    ...(missing.length > 0 ? [`no payload for: ${missing.join(", ")}`] : []),
    ...(stale.length > 0 ? [`payloads for unknown entries: ${stale.join(", ")}`] : []),
  ];
  if (problems.length > 0) throw new BenchError(`${args.payloads}: ${problems.join("; ")}`);
  const unknown = args.only.filter((id) => !ids.has(id));
  if (unknown.length > 0)
    throw new BenchError(`--only names unknown entries: ${unknown.join(", ")}`);
  return args.only.length === 0 ? [...entries] : entries.filter((e) => args.only.includes(e.id));
}

/** argv for one spawn of `entry`, with the `sh` launcher hosts pay for. */
function entryArgv(entry: HookEntry, env: Env): { argv: string[]; implementation: "bun" | "rust" } {
  const resolved = resolveEntryCommand(
    {
      plugin: entry.plugin,
      entry: entry.entry,
      bundle: join(entry.root, "hooks", "dist", `${entry.entry}.js`),
      defaultArgv: ["/bin/sh", "-c", entry.command],
    },
    env,
  );
  return resolved.implementation === "bun"
    ? resolved
    : { argv: ["/bin/sh", "-c", 'exec "$0" "$@"', ...resolved.argv], implementation: "rust" };
}

/** Take `warmup + runs` measurements one after another and keep the last `runs`. */
async function samplesOf(
  measurer: string,
  spawn: Spawn,
  label: string,
  args: BenchArgs,
): Promise<MeasureReport[]> {
  const rounds = [...Array.from({ length: args.warmup + args.runs }).keys()];
  const all = await inSequence(rounds, () => measureOnce(measurer, spawn, label));
  return all.slice(args.warmup);
}

async function measureEntry(
  measurer: string,
  sb: Sandbox,
  entry: HookEntry,
  payloads: Payloads,
  args: BenchArgs,
  env: Env,
): Promise<EntryResult> {
  const { argv, implementation } = entryArgv(entry, env);
  const stdin = payloadStdin(payloads.entries[entry.id]?.stdin ?? {}, sb.project);
  const spawn = {
    argv,
    cwd: sb.project,
    env: pretoolEnv(sb, "claude", { CLAUDE_PLUGIN_ROOT: entry.root }),
    stdin,
  };
  const label = `${entry.id} [${implementation}]`;
  const samples = await samplesOf(measurer, spawn, label, args);
  return { entry: entry.id, event: entry.event, implementation, ...summarize(samples) };
}

/** `TOOLU_IMPL` for the run: the manifest's selector under `--assert`, the caller's otherwise. */
function implEnv(args: BenchArgs): { env: Env; ported: string[] } {
  if (!args.assert) return { env: process.env, ported: [] };
  try {
    const ported = readPorted(args.manifest);
    const selector = portSelector(ported);
    return { env: { ...process.env, TOOLU_IMPL: selector ?? undefined }, ported };
  } catch (error) {
    if (error instanceof PortsError) throw new BenchError(error.message, 2, { cause: error });
    throw error;
  }
}

async function measureAll(
  args: BenchArgs,
  entries: HookEntry[],
  payloads: Payloads,
  env: Env,
): Promise<HookResult> {
  const measurer = buildMeasurer(ROOT);
  using sb = await benchSandbox();
  const started = performance.now();
  const floorSpawn = { argv: ["true"], cwd: sb.project, env: {}, stdin: "" };
  const floor = summarize(await samplesOf(measurer, floorSpawn, "floor (true)", args));
  const rows = await inSequence(entries, (entry) =>
    measureEntry(measurer, sb, entry, payloads, args, env),
  );
  const provenance = {
    date: today(),
    commit: headCommit(ROOT),
    platform: platform(),
    arch: arch(),
    cpu: cpus()[0]?.model ?? "unknown",
    bun: Bun.version,
    runner:
      process.env.GITHUB_ACTIONS === "true" ? ("github-actions" as const) : ("local" as const),
    runs: args.runs,
    warmup: args.warmup,
    elapsedMs: Math.round(performance.now() - started),
    floor: { maxRssBytes: floor.maxRssBytes.p50, cpuUs: floor.cpuUs.p50, wallUs: floor.wallUs.p50 },
  };
  return { schema: "toolu.hook-resources/v1", provenance, entries: rows };
}

function assertBudgets(
  args: BenchArgs,
  result: HookResult,
  ported: string[],
  known: ReadonlySet<string>,
): string[] {
  const absent = ported.filter((id) => !known.has(id));
  if (absent.length > 0)
    throw new BenchError(`ported entries with no hooks.json launcher: ${absent.join(", ")}`);
  if (ported.length === 0) {
    process.stdout.write("hook-bench: no ported entries; nothing to assert\n");
    return [];
  }
  const measured = new Set(result.entries.map((row) => row.entry));
  const asserted = new Set(ported.filter((id) => measured.has(id)));
  const found = violations(result.entries, asserted, loadJson(args.budgets, Budgets), args.budgets);
  if (result.provenance.elapsedMs > MAX_ELAPSED_MS) {
    found.push(
      `measured phase took ${String(result.provenance.elapsedMs)} ms > ${String(MAX_ELAPSED_MS)} ms`,
    );
  }
  return found;
}

export async function main(argv: readonly string[]): Promise<number> {
  try {
    const args = parseArgs(argv, ROOT);
    const all = discoverEntries(join(ROOT, "plugins"));
    const payloads = loadJson(args.payloads, Payloads);
    const entries = selected(all, payloads, args);
    const { env, ported } = implEnv(args);
    const result = await measureAll(args, entries, payloads, env);
    process.stdout.write(
      `Platform: ${result.provenance.platform} ${result.provenance.arch} (${result.provenance.cpu}); Bun ${result.provenance.bun}; ` +
        `${String(args.runs)} runs after ${String(args.warmup)} warm-up; measurer floor ${(result.provenance.floor.maxRssBytes / 1048576).toFixed(1)} MiB; ` +
        `measured in ${(result.provenance.elapsedMs / 1000).toFixed(1)} s\n\n${table(result.entries)}`,
    );
    if (args.out !== undefined) {
      mkdirSync(dirname(args.out), { recursive: true });
      writeFileSync(args.out, `${JSON.stringify(result, null, 2)}\n`);
    }
    if (!args.assert) return 0;
    const found = assertBudgets(args, result, ported, new Set(all.map((e) => e.id)));
    for (const line of found) process.stderr.write(`hook-bench: ${line}\n`);
    return found.length === 0 ? 0 : 1;
  } catch (error) {
    if (!(error instanceof BenchError)) throw error;
    process.stderr.write(`hook-bench: ${error.message}\n`);
    return error.exitCode;
  }
}

if (import.meta.main) process.exit(await main(process.argv.slice(2)));
