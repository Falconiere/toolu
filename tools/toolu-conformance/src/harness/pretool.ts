/**
 * PreToolUse parity harness (#258): runs one hook call through the bash
 * dispatcher (`pre-tools/mod.sh`, or a standalone script) and through the
 * committed TypeScript bundle behind its generated launcher, as Claude Code or
 * Codex would spawn them, from the same sandbox state. Gates write state (gate
 * files, telemetry), so the sandbox is snapshotted before the first run and
 * restored, at the same paths, before the second.
 */
import { cpSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import type { Sandbox } from "./sandbox.ts";
import { hostEnv, run, type EnvPatch, type RunResult } from "./spawn.ts";

export type PretoolHost = "claude" | "codex";

export const REPO_ROOT = resolve(import.meta.dir, "../../../..");
export const TOOLU_PLUGIN = join(REPO_ROOT, "plugins", "toolu");

/** The hooks.json entry and the bash command it replaced. */
export const PRETOOL_ENTRIES = {
  "pre-tools": ["bash", join(TOOLU_PLUGIN, "hooks/pre-tools/mod.sh")],
  "pre-tools-mcp": [join(TOOLU_PLUGIN, "hooks/pre-tools/modules/mcp-blocker.sh")],
  "pre-tools-agent": [join(TOOLU_PLUGIN, "hooks/pre-tools/agent-tier.sh")],
} as const;

export type PretoolEntry = keyof typeof PRETOOL_ENTRIES;

/**
 * The host's hook environment, without the harness's host override so the
 * dispatcher detects the host as it would in a real session. Codex exports
 * `CLAUDE_PLUGIN_ROOT` beside `PLUGIN_ROOT`, which the launcher reads.
 */
export function pretoolEnv(sb: Sandbox, host: PretoolHost, extra: EnvPatch = {}): EnvPatch {
  return {
    ...hostEnv(host, sb, TOOLU_PLUGIN),
    TOOLU_HOST_OVERRIDE: undefined,
    CLAUDE_PLUGIN_ROOT: TOOLU_PLUGIN,
    TOOLU_BUN: process.execPath,
    ...extra,
  };
}

/** The host's user config root in the sandbox. */
export function hostConfigRoot(sb: Sandbox, host: PretoolHost): string {
  return host === "codex" ? sb.codexHome : sb.configDir("claude", "user");
}

/** Mark plugin specs installed where Claude Code records them (Codex reads its own snapshot). */
export function installPlugins(sb: Sandbox, ...specs: string[]): void {
  const file = join(sb.configDir("claude", "user"), "plugins", "installed_plugins.json");
  mkdirSync(dirname(file), { recursive: true });
  const plugins = Object.fromEntries(specs.map((spec) => [spec, [{ scope: "user" }]]));
  writeFileSync(file, `${JSON.stringify({ version: 2, plugins })}\n`);
}

/** Run a plugin's real SessionStart `register.sh`, syncing its modules into the registry. */
export async function registerPlugin(
  sb: Sandbox,
  host: PretoolHost,
  plugin: string,
): Promise<void> {
  const result = await run(["bash", join(REPO_ROOT, "plugins", plugin, "hooks/register.sh")], {
    cwd: sb.project,
    env: pretoolEnv(sb, host),
    stdin: "{}",
  });
  if (result.exitCode !== 0) {
    throw new Error(`register ${plugin} exited ${String(result.exitCode)}: ${result.stderr}`);
  }
}

export type PretoolRun = { cwd: string; env: EnvPatch; stdin: string };

/** The bash hook command hooks.json ran before #258. */
export function runBashEntry(entry: PretoolEntry, call: PretoolRun): Promise<RunResult> {
  return run([...PRETOOL_ENTRIES[entry]], call);
}

/** The hooks.json launcher for `entry`, which execs the committed bundle. */
export function runBundleEntry(entry: PretoolEntry, call: PretoolRun): Promise<RunResult> {
  const command = launcherCommand({ plugin: "toolu", event: "PreToolUse", entry });
  return run(["/bin/sh", "-c", command], call);
}

/** Run `first`, put the sandbox back exactly as it was, then run `second`. */
export async function fromSameState<T>(
  sb: Sandbox,
  first: () => Promise<T>,
  second: () => Promise<T>,
): Promise<[T, T]> {
  const saved = `${sb.root}.snapshot`;
  cpSync(sb.root, saved, { recursive: true, verbatimSymlinks: true });
  try {
    const a = await first();
    rmSync(sb.root, { recursive: true, force: true });
    cpSync(saved, sb.root, { recursive: true, verbatimSymlinks: true });
    return [a, await second()];
  } finally {
    rmSync(saved, { recursive: true, force: true });
  }
}
