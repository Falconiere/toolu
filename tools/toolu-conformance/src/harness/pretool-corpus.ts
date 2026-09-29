/**
 * The PreToolUse fixture corpus (#258): at least one fixture per decision path
 * of every toolu PreToolUse module, the ast-grep registry module and the
 * dispatcher itself, each with the sandbox state it needs. Triggers come from
 * the modules' bats suites. `expect` is the decision class on each host: Codex
 * cannot prompt, so a guardrail's ask is a deny there and a judgement gate's
 * ask is advice.
 */
import { cpSync } from "node:fs";
import { join } from "node:path";
import { writeFile, type PretoolCase } from "./pretool-case.ts";
import { EDIT_CASES } from "./pretool-cases-edit.ts";
import { SHELL_CASES } from "./pretool-cases-shell.ts";
import { installPlugins, registerPlugin, TOOLU_PLUGIN, type PretoolHost } from "./pretool.ts";
import type { Sandbox } from "./sandbox.ts";
import type { EnvPatch } from "./spawn.ts";

export type { Outcome, PretoolCase } from "./pretool-case.ts";

export const PRETOOL_CORPUS: readonly PretoolCase[] = [...EDIT_CASES, ...SHELL_CASES];

const TOOL_REGISTRY_SPECS = ["toolu@toolu", "ast-grep@toolu", "fixture@toolu"];

/** Settings: a copy of the shipped settings with `extra` lines appended. */
function settingsDir(sb: Sandbox, extra: Record<string, string>): string {
  const dir = join(sb.root, "settings");
  cpSync(join(TOOLU_PLUGIN, "settings"), dir, { recursive: true });
  for (const [file, lines] of Object.entries(extra)) writeFile(join(dir, file), lines);
  return dir;
}

/**
 * Put the sandbox in `c`'s state for `host`: the ast-grep module registered by
 * its real `register.sh`, plugins installed, config, settings and extra files.
 * Returns the env additions the case needs.
 */
export async function prepare(sb: Sandbox, host: PretoolHost, c: PretoolCase): Promise<EnvPatch> {
  // No detached gc/maintenance after a case's own commits: it would write into
  // `.git` while the sandbox is snapshotted and compared.
  sb.git("config", "maintenance.auto", "false");
  sb.git("config", "gc.auto", "0");
  installPlugins(sb, ...TOOL_REGISTRY_SPECS);
  await registerPlugin(sb, host, "ast-grep");
  if (c.config !== undefined) sb.writeConfig(host, "project", c.config);
  c.setup?.(sb, host);
  return c.settings === undefined ? {} : { TOOLU_SETTINGS_DIR: settingsDir(sb, c.settings) };
}
