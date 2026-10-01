/** Native dispatcher fixtures with an isolated registry module. */
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import {
  dispatchPreTool,
  type HookPhase,
  type ModuleResult,
  type ToolModule,
} from "../dispatch.ts";

export const LIB = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib");

export function registryDir(sb: Sandbox, phase: HookPhase = "pre"): string {
  return join(sb.home, ".claude", "toolu", phase === "pre" ? "pre-tools.d" : "post-tools.d");
}

/** Write an executable bash module whose body follows the shebang. */
export function writeModule(dir: string, file: string, body: string): void {
  mkdirSync(dir, { recursive: true });
  const path = join(dir, file);
  writeFileSync(path, `#!/usr/bin/env bash\n${body}\n`);
  chmodSync(path, 0o755);
}

function installedFile(sb: Sandbox): string {
  return join(sb.home, ".claude", "plugins", "installed_plugins.json");
}

function installedSpecs(sb: Sandbox): string[] {
  const file = installedFile(sb);
  if (!existsSync(file)) return [];
  const doc: unknown = JSON.parse(readFileSync(file, "utf8"));
  const plugins = typeof doc === "object" && doc !== null ? Reflect.get(doc, "plugins") : undefined;
  return typeof plugins === "object" && plugins !== null ? Object.keys(plugins) : [];
}

/** Record `specs` as installed in Claude's plugin registry, beside those already recorded. */
export function install(sb: Sandbox, ...specs: string[]): void {
  const file = installedFile(sb);
  mkdirSync(dirname(file), { recursive: true });
  const all = [...new Set([...installedSpecs(sb), ...specs])];
  const plugins = Object.fromEntries(all.map((spec) => [spec, [{ scope: "user" }]]));
  writeFileSync(file, JSON.stringify({ version: 2, plugins }));
}

/**
 * A bash module that runs first in the walk: `builtin@fixture__<file>` in the
 * registry, installed. Its own spec, so a test's `fixture@toolu` ESM module
 * does not shadow it, and it sorts before every other test spec.
 */
export function writeBuiltin(
  sb: Sandbox,
  file: string,
  body: string,
  phase: HookPhase = "pre",
): void {
  writeModule(registryDir(sb, phase), `builtin@fixture__${file}`, body);
  install(sb, "builtin@fixture");
}

export function hookEnv(sb: Sandbox, extra: Record<string, string> = {}): Record<string, string> {
  return {
    PATH: process.env.PATH ?? "/usr/bin:/bin",
    HOME: sb.home,
    CLAUDE_PROJECT_DIR: sb.project,
    ...extra,
  };
}

export function runTsDispatch(
  stdin: string,
  env: Record<string, string>,
  builtins: readonly ToolModule[] = [],
): Promise<ModuleResult> {
  return dispatchPreTool(stdin, { builtins, libDir: LIB, env });
}
