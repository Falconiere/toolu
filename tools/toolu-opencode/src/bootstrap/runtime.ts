/**
 * Complete plugin startup on OpenCode (#342). Every selected plugin runs every
 * SessionStart entry of its `hooks.json`, dependencies first; each entry
 * reports its registry and helper contributions, which are verified on disk
 * before they count. Plugins that are no longer selected lose what toolu
 * published for them. Readiness is this run's verdict only.
 */
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { opencodeDataRoot } from "../host/roots.ts";
import { listPluginManifests } from "../inventory/scan.ts";
import type { PluginManifest } from "../inventory/types.ts";
import { resolveBunExecutable } from "../preflight/check.ts";
import { pluginStartupEntries, type StartupEntry } from "./entrypoint.ts";
import { emptyLedger, pruneUnselected, readLedger, type Cleanup, type Ledger } from "./ledger.ts";
import { startupOrder } from "./order.ts";
import { parseStartupOutput } from "./output.ts";
import { settle, type PluginRun } from "./readiness.ts";
import { readStartupReport, verifyRecords, type Verified } from "./records.ts";
import { notReady, type BootstrapResult, type EntryOutcome } from "./result.ts";
import { spawnEntry } from "./spawn.ts";

export type BootstrapRuntimeOptions = {
  repoRoot: string;
  dataRoot?: string;
  projectRoot: string;
  plugins: PluginManifest[];
  env?: Record<string, string>;
  isolatedHome?: string;
  /** Per entry; default 120 s. */
  deadlineMs?: number;
  /** Cancels the whole startup: the running entry is killed and startup is NotReady. */
  signal?: AbortSignal;
};

/**
 * `STARTUP_REPORT_ENV` of `@toolu/core/startup`, spelled here: the npm route
 * installs `@toolu/core` from the registry, where the release that adds the
 * export may not be the one installed. A test pins the two together.
 */
export const STARTUP_REPORT_VAR = "TOOLU_STARTUP_REPORT";

type Run = {
  bun: string;
  env: Record<string, string>;
  projectRoot: string;
  dataRoot: string;
  deadlineMs: number;
  signal: AbortSignal | undefined;
};

/** At most this much of a failing entry's output goes into the reason. */
const EXCERPT_CHARS = 500;

function excerpt(text: string): string {
  const trimmed = text.trim();
  return trimmed.length > EXCERPT_CHARS ? `${trimmed.slice(0, EXCERPT_CHARS)}…` : trimmed;
}

function bootstrapEnv(options: BootstrapRuntimeOptions, dataRoot: string): Record<string, string> {
  const inherited: Record<string, string> = {};
  for (const [key, value] of Object.entries(process.env)) {
    if (value !== undefined) inherited[key] = value;
  }
  return {
    ...inherited,
    ...options.env,
    TOOLU_CONFIG_DIR: dataRoot,
    TOOLU_PROJECT_DIR: options.projectRoot,
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    TOOLU_HOST_OVERRIDE: "opencode",
    HOME: options.isolatedHome ?? dataRoot,
  };
}

/** Run one entry against a fresh report file; returns its outcome or why it failed. */
async function runEntry(
  entry: StartupEntry,
  plugin: PluginManifest,
  run: Run,
  verified: Verified,
): Promise<EntryOutcome | string> {
  const dir = mkdtempSync(join(tmpdir(), "toolu-startup-"));
  try {
    const report = join(dir, "report.jsonl");
    writeFileSync(report, "");
    const env = { ...run.env, CLAUDE_PLUGIN_ROOT: plugin.pluginDir, [STARTUP_REPORT_VAR]: report };
    const stdin = JSON.stringify({
      hook_event_name: "SessionStart",
      source: "startup",
      cwd: run.projectRoot,
    });
    const spawned = await spawnEntry({
      bun: run.bun,
      bundle: entry.bundle,
      cwd: run.projectRoot,
      env,
      stdin,
      deadlineMs: run.deadlineMs,
      signal: run.signal,
    });
    // Read the report whatever the exit: a helper published before a crash is still toolu's.
    const read = readStartupReport(report);
    const found = read.ok ? verifyRecords(read.records, plugin, run.dataRoot) : undefined;
    if (found !== undefined) {
      verified.artifacts.push(...found.artifacts);
      verified.helpers.push(...found.helpers);
      verified.diagnostics.push(...found.diagnostics);
    }
    if (spawned.status === "failed") return spawned.reason;
    if (spawned.exitCode !== 0) {
      const said = excerpt(spawned.stderr || spawned.stdout);
      return said === "" ? `exited ${spawned.exitCode}` : `exited ${spawned.exitCode}: ${said}`;
    }
    const output = parseStartupOutput(spawned.stdout);
    if (!output.ok) return output.reason;
    if (!read.ok) return read.reason;
    if (found !== undefined && found.failures.length > 0) return found.failures.join("; ");
    const stderr = excerpt(spawned.stderr);
    if (stderr !== "") verified.diagnostics.push(`${plugin.name}/${entry.name}: ${stderr}`);
    return { entry: entry.name, ...output.context };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** Entries run one after another: a later entry may rely on an earlier one's effects. */
type Progress = { verified: Verified; outcomes: EntryOutcome[] };

async function runEntries(
  entries: readonly StartupEntry[],
  plugin: PluginManifest,
  run: Run,
  done: Progress,
): Promise<string | undefined> {
  const [entry, ...rest] = entries;
  if (entry === undefined) return undefined;
  const outcome = await runEntry(entry, plugin, run, done.verified);
  if (typeof outcome === "string") return `${plugin.name}/${entry.name}: ${outcome}`;
  done.outcomes.push(outcome);
  return runEntries(rest, plugin, run, done);
}

function noContributions(): Verified {
  return { artifacts: [], helpers: [], diagnostics: [], failures: [] };
}

async function startPlugin(plugin: PluginManifest, run: Run): Promise<PluginRun> {
  const verified = noContributions();
  const plan = pluginStartupEntries(plugin.pluginDir);
  if (!plan.ok)
    return { status: "failed", plugin, verified, failure: `${plugin.name}: ${plan.reason}` };
  const done: Progress = { verified, outcomes: [] };
  const failure = await runEntries(plan.entries, plugin, run, done);
  if (failure !== undefined) return { status: "failed", plugin, verified, failure };
  return { status: "ready", plugin, verified, entries: done.outcomes };
}

/** Plugins in startup order; a plugin whose dependency failed is skipped as failed. */
async function startAll(
  plugins: readonly PluginManifest[],
  run: Run,
  runs: PluginRun[] = [],
): Promise<PluginRun[]> {
  const [plugin, ...rest] = plugins;
  if (plugin === undefined) return runs;
  const failed = new Set(runs.filter((r) => r.status === "failed").map((r) => r.plugin.name));
  const dep = plugin.dependencies.find((d) => failed.has(d.name));
  runs.push(
    dep === undefined
      ? await startPlugin(plugin, run)
      : {
          status: "failed",
          plugin,
          verified: noContributions(),
          failure: `${plugin.name}: skipped, dependency ${dep.name} failed`,
        },
  );
  return startAll(rest, run, runs);
}

async function bootstrapRuntimeInternal(
  options: BootstrapRuntimeOptions,
): Promise<BootstrapResult> {
  const dataRoot = options.dataRoot ?? opencodeDataRoot({ projectRoot: options.projectRoot });
  mkdirSync(dataRoot, { recursive: true });
  const env = bootstrapEnv(options, dataRoot);
  // The child HOME is isolated; the runtime must come from the original host HOME.
  const bun = resolveBunExecutable({ ...env, HOME: options.env?.HOME ?? process.env.HOME ?? "" });
  if (!bun) return notReady("Bun runtime not found, checked TOOLU_BUN, PATH and ~/.bun/bin/bun");
  const order = startupOrder(options.plugins);
  if (!order.ok) return notReady(order.reason);
  const cleanup: Cleanup = { failures: [], diagnostics: [] };
  const read = readLedger(dataRoot);
  if (read.diagnostic !== undefined) cleanup.diagnostics.push(read.diagnostic);
  const ledger: Ledger = read.ledger;
  const retained = pruneUnselected(
    {
      dataRoot,
      selected: new Set(order.plugins.map((p) => p.name)),
      catalog: listPluginManifests(join(options.repoRoot, "plugins")) ?? [],
      ledger,
    },
    cleanup,
  );
  const run: Run = {
    bun,
    env,
    projectRoot: options.projectRoot,
    dataRoot,
    deadlineMs: options.deadlineMs ?? 120_000,
    signal: options.signal,
  };
  const runs = await startAll(order.plugins, run);
  return settle({
    dataRoot,
    runs,
    previous: ledger,
    next: { ...emptyLedger(), plugins: retained },
    cleanup,
  });
}

/** All startup failures become NotReady so the plugin can deny every tool call. */
export async function bootstrapRuntime(options: BootstrapRuntimeOptions): Promise<BootstrapResult> {
  try {
    return await bootstrapRuntimeInternal(options);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return notReady(`bootstrap failed: ${reason}`);
  }
}
