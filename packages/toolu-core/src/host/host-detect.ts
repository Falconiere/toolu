/** Host detection (#252): which host spawned this hook. */
import { hostsForNativeEvent } from "./host-events.ts";
import { envValue, isHostName, type HostEnv, type HostName } from "./host-name.ts";

export type DetectOptions = {
  env?: HostEnv;
  /** stdin `hook_event_name`, when the caller has already read it. */
  hookEventName?: string;
  /** Set by the in-process OpenCode adapter, which has no hook subprocess. */
  inProcess?: "opencode";
  /** Receives the invalid-override warning; default writes it to stderr. */
  warn?: (line: string) => void;
};

function stderrLine(line: string): void {
  process.stderr.write(`${line}\n`);
}

/**
 * Resolve the host, first hit wins: `TOOLU_HOST_OVERRIDE`, the in-process
 * OpenCode flag, a stdin event name only one host uses, Cursor's per-hook
 * variables, Codex's `PLUGIN_ROOT`, then Claude. Cursor precedes Codex because
 * Cursor sets its variables on every hook while `PLUGIN_ROOT` is not Codex-only
 * by contract. Hermes documents no hook environment, so only its event names
 * (or the override) select it; `HERMES_HOME` is a user-global profile variable.
 */
export function detectHost(options: DetectOptions = {}): HostName {
  const env = options.env ?? process.env;
  const override = envValue(env, "TOOLU_HOST_OVERRIDE");
  if (override !== undefined) {
    if (isHostName(override)) {
      return override;
    }
    (options.warn ?? stderrLine)(
      `toolu-host: invalid TOOLU_HOST_OVERRIDE '${override}' (using environment detection)`,
    );
  }
  if (options.inProcess === "opencode") {
    return "opencode";
  }
  const owners = options.hookEventName ? hostsForNativeEvent(options.hookEventName) : [];
  const [owner] = owners;
  if (owners.length === 1 && owner !== undefined) {
    return owner;
  }
  if (envValue(env, "CURSOR_VERSION") ?? envValue(env, "CURSOR_PROJECT_DIR")) {
    return "cursor";
  }
  return envValue(env, "PLUGIN_ROOT") ? "codex" : "claude";
}
