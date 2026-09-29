/**
 * Runs one PreToolUse call through both dispatchers (#258): bash, as
 * `pre-tools/mod.sh` does it but over a test modules directory, and
 * `dispatchPreTool` over the same directory as a table of `bashModule`s.
 */
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { bashModule, dispatchPreTool, type ModuleResult, type PreToolModule } from "../dispatch.ts";

export const LIB = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib");

/** `pre-tools/mod.sh`, with the modules directory as `$1` instead of fixed. */
const MOD_SH = `
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

export function modulesDir(sb: Sandbox): string {
  return join(sb.root, "modules");
}

export function registryDir(sb: Sandbox): string {
  return join(sb.home, ".claude", "toolu", "pre-tools.d");
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
export function tableOf(dir: string): PreToolModule[] {
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
): ModuleResult {
  const res = spawnSync("bash", ["-c", MOD_SH, "mod.sh", modulesDir(sb)], {
    cwd: sb.project,
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
  builtins: readonly PreToolModule[] = tableOf(modulesDir(sb)),
): Promise<ModuleResult> {
  return dispatchPreTool(stdin, { builtins, libDir: LIB, env });
}
