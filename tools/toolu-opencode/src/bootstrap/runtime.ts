/**
 * Complete plugin startup on OpenCode (#342). Every selected plugin runs every
 * SessionStart entry of its `hooks.json`, dependencies first; each entry
 * reports its registry and helper contributions, which are verified on disk
 * before they count. Plugins that are no longer selected lose what toolu
 * published for them. Readiness is this run's verdict only. Entries run with
 * the user's HOME and toolu's roots, never another host's (#343).
 */
import { randomUUID } from "node:crypto";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  OPENCODE_STARTUP_LEDGER,
  opencodeConfigRoot,
  opencodeDataRoot,
  opencodeLegacySharedRoot,
  opencodeRegistryRoot,
} from "../host/roots.ts";
import { definedEnv, tooluProcessEnv } from "../host/runtime-env.ts";
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
  /** Host env, over `process.env`; its HOME and overrides decide the roots. */
  env?: Record<string, string>;
  /** Global `toolu.config.json` directory; default `opencodeConfigRoot` of the env. */
  userConfigRoot?: string;
  /** Tests only: a HOME for the entries instead of the user's own. */
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

function hostEnv(options: BootstrapRuntimeOptions): Record<string, string> {
  return { ...definedEnv(process.env), ...options.env };
}

function bootstrapEnv(
  options: BootstrapRuntimeOptions,
  host: Record<string, string>,
  dataRoot: string,
  bun: string,
): Record<string, string> {
  const roots = {
    projectRoot: options.projectRoot,
    dataRoot,
    userConfigRoot: options.userConfigRoot ?? opencodeConfigRoot({ env: host }),
    repoRoot: options.repoRoot,
  };
  const env = { ...tooluProcessEnv(host, roots), TOOLU_BUN: bun, TOOLU_SESSION_ID: randomUUID() };
  return options.isolatedHome === undefined ? env : { ...env, HOME: options.isolatedHome };
}

/** The in-session hint for an override still laid out as the pre-#343 shared root. */
function legacySharedNote(host: Record<string, string>, dataRoot: string): string | undefined {
  const legacy = opencodeLegacySharedRoot({ env: host });
  if (legacy === undefined || legacy === opencodeRegistryRoot(dataRoot)) return undefined;
  const projects = join(legacy, "opencode", "projects");
  const ledger = join(legacy, OPENCODE_STARTUP_LEDGER);
  return `shared data root ${legacy} from before #343 is no longer used; each project now starts in ${projects}. Delete ${ledger} (OpenCode's own) to silence this note; see docs/opencode.md, Roots and helper environment`;
}

/** An empty report file in a new temp directory, or why there cannot be one. */
function freshReport(): { ok: true; dir: string; report: string } | { ok: false; reason: string } {
  let dir: string;
  try {
    dir = mkdtempSync(join(tmpdir(), "toolu-startup-"));
  } catch (error) {
    return { ok: false, reason: `cannot create a startup report directory: ${String(error)}` };
  }
  const report = join(dir, "report.jsonl");
  try {
    writeFileSync(report, "");
    return { ok: true, dir, report };
  } catch (error) {
    rmSync(dir, { recursive: true, force: true });
    return { ok: false, reason: `cannot create startup report ${report}: ${String(error)}` };
  }
}

/** Run one entry against a fresh report file; returns its outcome or why it failed. */
async function runEntry(
  entry: StartupEntry,
  plugin: PluginManifest,
  run: Run,
  verified: Verified,
): Promise<EntryOutcome | string> {
  const fresh = freshReport();
  if (!fresh.ok) return fresh.reason;
  const { dir, report } = fresh;
  try {
    const env = {
      ...run.env,
      CLAUDE_PLUGIN_ROOT: plugin.pluginDir,
      TOOLU_PLUGIN_ROOT: plugin.pluginDir,
      [STARTUP_REPORT_VAR]: report,
    };
    const stdin = JSON.stringify({
      hook_event_name: "SessionStart",
      source: "startup",
      cwd: run.projectRoot,
    });
    const spawned = await spawnEntry({
      bun: run.bun,
      bundle: entry.bundle,
      ...(entry.command === undefined ? {} : { command: entry.command }),
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
    return {
      entry: entry.name,
      ...(entry.command === undefined ? {} : { native: true as const }),
      ...output.context,
    };
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
  const host = hostEnv(options);
  const dataRoot =
    options.dataRoot ?? opencodeDataRoot({ projectRoot: options.projectRoot, env: host });
  mkdirSync(dataRoot, { recursive: true });
  const bun = resolveBunExecutable(host);
  if (!bun) return notReady("Bun runtime not found, checked TOOLU_BUN, PATH and ~/.bun/bin/bun");
  const env = bootstrapEnv(options, host, dataRoot, bun);
  const order = startupOrder(options.plugins);
  if (!order.ok) return notReady(order.reason);
  const cleanup: Cleanup = { failures: [], diagnostics: [] };
  const read = readLedger(dataRoot);
  if (read.diagnostic !== undefined) cleanup.diagnostics.push(read.diagnostic);
  const legacy = legacySharedNote(host, dataRoot);
  if (legacy !== undefined) cleanup.diagnostics.push(legacy);
  const ledger: Ledger = read.ledger;
  const pluginsRoot = join(options.repoRoot, "plugins");
  const catalog = listPluginManifests(pluginsRoot);
  if (catalog === null) {
    cleanup.diagnostics.push(
      `plugin catalog ${pluginsRoot} unreadable; only plugins in the startup ledger were pruned`,
    );
  }
  const selected = new Set(order.plugins.map((p) => p.name));
  const prune = { dataRoot, selected, catalog: catalog ?? [], ledger };
  const retained = pruneUnselected(prune, cleanup);
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
