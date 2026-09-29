/** Real events and contexts for driving a native gate in process (#260). */
import { join, resolve } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import type { HostName } from "../../host/host-name.ts";
import type { RegistryContext, RegistryHookEvent } from "../../registry/registry-types.ts";

export const SHIPPED_SETTINGS = resolve(import.meta.dir, "../../../../../plugins/toolu/settings");

/** The env a hook sees in `sb`: sandbox HOME and project, the given settings directory. */
export function gateEnv(sb: Sandbox, settings: string, extra: Record<string, string> = {}) {
  return {
    PATH: process.env.PATH ?? "/usr/bin:/bin",
    HOME: sb.home,
    TOOLU_PROJECT_DIR: sb.project,
    TOOLU_CONFIG_DIR: join(sb.home, ".claude"),
    TOOLU_SETTINGS_DIR: settings,
    ...extra,
  };
}

function base(sb: Sandbox, toolName: string, toolInput: Record<string, unknown>) {
  return {
    sessionId: "s",
    cwd: sb.project,
    projectRoot: sb.project,
    worktree: sb.project,
    toolCallId: "c",
    toolName,
    toolInput,
  };
}

export function shellEvent(sb: Sandbox, command: string, toolName = "Bash"): RegistryHookEvent {
  return { ...base(sb, toolName, { command }), type: "shell/pre", command };
}

export function toolEvent(
  sb: Sandbox,
  toolName: string,
  toolInput: Record<string, unknown>,
): RegistryHookEvent {
  return { ...base(sb, toolName, toolInput), type: "tool/pre" };
}

/** `cwd`, when given, is the hook process's working directory (`ctx.cwd`). */
export function gateCtx(
  sb: Sandbox,
  host: HostName,
  env: Record<string, string>,
  cwd?: string,
): RegistryContext {
  const ctx = { host, env, configRoot: join(sb.home, ".claude"), projectRoot: sb.project, raw: {} };
  return cwd === undefined ? ctx : { ...ctx, cwd };
}
