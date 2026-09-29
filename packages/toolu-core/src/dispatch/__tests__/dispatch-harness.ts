/**
 * Runs one PreToolUse (#258) or PostToolUse (#259) call through both
 * dispatchers: bash, as `pre-tools/mod.sh` / `post-tools/mod.sh` do it but over
 * a test modules directory, and `dispatchPreTool` / `dispatchPostTool` over the
 * same directory as a table of `bashModule`s.
 */
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import {
  bashModule,
  dispatchPostTool,
  dispatchPreTool,
  type HookPhase,
  type ModuleResult,
  type ToolModule,
} from "../dispatch.ts";

export const LIB = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib");

/** `pre-tools/mod.sh`, with the modules directory as `$1` instead of fixed. */
const PRE_MOD_SH = `
HOOK_LIB="${LIB}"
. "$HOOK_LIB/config.sh"; . "$HOOK_LIB/dispatch.sh"; . "$HOOK_LIB/detect.sh"; . "$HOOK_LIB/registry.sh"
if ! toolu_enabled hooks pre-tools; then cat > /dev/null 2>&1 || true; exit 0; fi
export TOOLU_LIB_DIR="$HOOK_LIB"
TOOLU_CONFIG_DIR="$(toolu_config_root)"; export TOOLU_CONFIG_DIR
input=$(cat)
tool_name=$(jq -r '.tool_name // ""' <<<"$input" 2>/dev/null || echo "")
export input tool_name
toolu_dispatch_hook "$1" "PreToolUse" "$(toolu_registry_event_dir PreToolUse)"
`;

/** `post-tools/mod.sh`, with the modules directory as `$1` instead of fixed. */
const POST_MOD_SH = `
HOOK_LIB="${LIB}"
. "$HOOK_LIB/config.sh"; . "$HOOK_LIB/dispatch.sh"; . "$HOOK_LIB/detect.sh"; . "$HOOK_LIB/registry.sh"
if ! toolu_enabled hooks post-tools; then cat > /dev/null 2>&1 || true; exit 0; fi
export TOOLU_LIB_DIR="$HOOK_LIB"
TOOLU_CONFIG_DIR="$(toolu_config_root)"; export TOOLU_CONFIG_DIR
input=$(cat 2>/dev/null || echo "{}")
tool_name=$(jq -r '.tool_name // ""' <<<"$input" 2>/dev/null || echo "")
PROJECT_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
export PATH="$PROJECT_ROOT/node_modules/.bin:$PATH"
export input tool_name PROJECT_ROOT
toolu_dispatch_hook "$1" "PostToolUse" "$(toolu_registry_event_dir PostToolUse)"
`;

export function modulesDir(sb: Sandbox): string {
  return join(sb.root, "modules");
}

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

/** Record `spec` as installed in Claude's plugin registry. */
export function install(sb: Sandbox, ...specs: string[]): void {
  const file = join(sb.home, ".claude", "plugins", "installed_plugins.json");
  mkdirSync(dirname(file), { recursive: true });
  const plugins = Object.fromEntries(specs.map((spec) => [spec, [{ scope: "user" }]]));
  writeFileSync(file, JSON.stringify({ version: 2, plugins }));
}

export function hookEnv(sb: Sandbox, extra: Record<string, string> = {}): Record<string, string> {
  return {
    PATH: process.env.PATH ?? "/usr/bin:/bin",
    HOME: sb.home,
    CLAUDE_PROJECT_DIR: sb.project,
    ...extra,
  };
}

/** Every `*.sh` in the modules directory, in the byte order bash globs them. */
export function tableOf(dir: string): ToolModule[] {
  let names: string[] = [];
  try {
    names = readdirSync(dir).filter((file) => file.endsWith(".sh"));
  } catch {
    names = [];
  }
  return names
    .toSorted((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)))
    .map((file) => bashModule(dir, file.slice(0, -".sh".length)));
}

export function runBashDispatch(
  sb: Sandbox,
  stdin: string,
  env: Record<string, string>,
  phase: HookPhase = "pre",
  cwd: string = sb.project,
): ModuleResult {
  const script = phase === "pre" ? PRE_MOD_SH : POST_MOD_SH;
  const res = spawnSync("bash", ["-c", script, "mod.sh", modulesDir(sb)], {
    cwd,
    env,
    input: stdin,
    encoding: "utf8",
  });
  return { stdout: res.stdout, stderr: res.stderr, exitCode: res.status ?? -1 };
}

export function runTsDispatch(
  sb: Sandbox,
  stdin: string,
  env: Record<string, string>,
  builtins: readonly ToolModule[] = tableOf(modulesDir(sb)),
): Promise<ModuleResult> {
  return dispatchPreTool(stdin, { builtins, libDir: LIB, env });
}

/** `dispatchPostTool` run from `cwd`, the way the hook process starts there. */
export function runTsPostDispatch(
  sb: Sandbox,
  stdin: string,
  env: Record<string, string>,
  cwd: string = sb.project,
): Promise<ModuleResult> {
  return dispatchPostTool(stdin, { builtins: tableOf(modulesDir(sb)), libDir: LIB, env, cwd });
}
