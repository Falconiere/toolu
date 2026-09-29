/**
 * PreToolUse parity harness (#258): runs one hook call through the bash
 * dispatcher (`pre-tools/mod.sh`) and through the
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

/** The bash command hooks.json ran before #258. */
export const MOD_SH = join(TOOLU_PLUGIN, "hooks/pre-tools/mod.sh");

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

/** `bash pre-tools/mod.sh`, the hooks.json command before #258. */
export function runModSh(call: PretoolRun): Promise<RunResult> {
  return run(["bash", MOD_SH], call);
}

/** The hooks.json launcher, which execs the committed `hooks/dist/pre-tools.js`. */
export function runBundle(call: PretoolRun): Promise<RunResult> {
  const command = launcherCommand({ plugin: "toolu", event: "PreToolUse", entry: "pre-tools" });
  return run(["/bin/sh", "-c", command], call);
}

function isErrno(error: unknown, code: string): boolean {
  return error instanceof Error && "code" in error && error.code === code;
}

/**
 * Copy `from` to `to`, retrying when a file vanishes mid-copy: git's detached
 * background maintenance (started by the sandbox's initial commit) holds a
 * transient `.git/objects/maintenance.lock` that can disappear between the
 * directory read and its stat. A lock is not repository state.
 */
function copyTree(from: string, to: string, attempts = 5): void {
  try {
    cpSync(from, to, { recursive: true, verbatimSymlinks: true });
  } catch (error) {
    if (!isErrno(error, "ENOENT") || attempts <= 1) throw error;
    rmSync(to, { recursive: true, force: true });
    copyTree(from, to, attempts - 1);
  }
}

/** Run `first`, put the sandbox back exactly as it was, then run `second`. */
export async function fromSameState<T>(
  sb: Sandbox,
  first: () => Promise<T>,
  second: () => Promise<T>,
): Promise<[T, T]> {
  const saved = `${sb.root}.snapshot`;
  copyTree(sb.root, saved);
  try {
    const a = await first();
    rmSync(sb.root, { recursive: true, force: true });
    copyTree(saved, sb.root);
    return [a, await second()];
  } finally {
    rmSync(saved, { recursive: true, force: true });
  }
}
