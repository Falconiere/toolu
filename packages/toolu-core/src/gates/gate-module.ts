/**
 * The shape of a native built-in gate (#260): the `native` variant of the
 * PreToolUse dispatcher's module table, shared with `RegistryModule.run`.
 */
import { loadConfig, type LoadedConfig } from "../config/config-load.ts";
import { settingsDir } from "../config/settings.ts";
import type { Decision } from "../decision/decision.ts";
import type { HostEvent } from "../host/host-events.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";

export type GateModule = {
  readonly kind: "native";
  readonly name: string;
  run(event: RegistryHookEvent, ctx: RegistryContext): Promise<Decision>;
};

export type GateModuleOptions = {
  /** The plugin root whose `settings/` is the last `settingsDir` fallback. */
  readonly pluginRoot?: string;
  /**
   * Where config warnings go. Silent by default: inside the dispatcher they
   * were already printed once, before any module ran.
   */
  readonly warn?: (line: string) => void;
};

/** The settings directory for `ctx`, as `toolu_settings_dir` resolves it. */
export function gateSettingsDir(
  ctx: RegistryContext,
  options: GateModuleOptions,
): string | undefined {
  return settingsDir({
    env: ctx.env,
    ...(options.pluginRoot === undefined ? {} : { pluginRoot: options.pluginRoot }),
  });
}

/** The merged config for `ctx`, warnings routed to `options.warn`. */
export function gateConfig(ctx: RegistryContext, options: GateModuleOptions): LoadedConfig {
  return loadConfig({ env: ctx.env, host: ctx.host, warn: options.warn ?? (() => undefined) });
}

/** The host event a pre-tool gate mode degrades `ask` for. */
export function preToolHostEvent(event: RegistryHookEvent): HostEvent {
  return event.type === "shell/pre" ? "shell/pre" : "tool/pre";
}

export const ALLOW: Decision = { kind: "allow" };

/** `.tool_input.<key> // ""` for a string field; any other value reads as absent. */
export function inputString(event: RegistryHookEvent, key: string): string {
  const value = event.toolInput[key];
  return typeof value === "string" ? value : "";
}
