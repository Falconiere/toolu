/**
 * One isolated OpenCode host session for a live probe (#335). Each session
 * owns a temp git project and a fresh HOME / XDG config, data and state root,
 * so the user's real OpenCode profile is never read or written, and only the
 * scripted provider is enabled, so no real model API can be called. The run-level
 * cache (XDG_CACHE_HOME and the npm cache) is shared across sessions so the
 * host's provider package and config-dir SDK install download once per run.
 */
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { startScriptedProvider, type RecordedRequest, type Scripts } from "./provider.ts";

export const PROBES_DIR = join(import.meta.dir, "../../../tools/toolu-opencode/contract/probes");
export const PROBE_PLUGIN = join(PROBES_DIR, "probe.ts");
export const DENY_MARKER = "DENYME";
export const DENY_MESSAGE = "toolu-probe: denied before execution";

export type SessionOptions = {
  /** Probe plugin files copied into `.opencode/plugins/` (local-file discovery). */
  localPlugins?: string[];
  /** Behavior for contract/probes/probe.ts (TOOLU_PROBE_CONFIG), given the sandbox root. */
  probeConfig?: (root: string) => Record<string, string>;
  /** Merged into the generated opencode.json (plugin, permission, mcp, …), given the sandbox root. */
  config?: (root: string) => Record<string, unknown>;
  scripts?: Scripts;
  /** Model id under the scripted provider; `gpt-*` ids get `apply_patch`. */
  model?: string;
  files?: Record<string, string>;
};

export type ProbeSession = {
  readonly sb: Sandbox;
  readonly env: Record<string, string>;
  readonly logPath: string;
  log(): Array<Record<string, unknown>>;
  requests(): RecordedRequest[];
  /** Whether `rel` exists in the project. */
  exists(rel: string): boolean;
  /** An absolute path under the sandbox root, outside the project the host sees. */
  outside(rel: string): string;
  [Symbol.dispose](): void;
};

function hostConfig(url: string, model: string, extra: Record<string, unknown>): object {
  return {
    $schema: "https://opencode.ai/config.json",
    model: `probe/${model}`,
    small_model: `probe/${model}`,
    share: "disabled",
    autoupdate: false,
    enabled_providers: ["probe"],
    provider: {
      probe: {
        npm: "@ai-sdk/openai-compatible",
        name: "probe",
        options: { baseURL: url, apiKey: "probe" },
        models: { [model]: { name: model, tool_call: true } },
      },
    },
    ...extra,
  };
}

function profileEnv(
  sb: Sandbox,
  cacheRoot: string,
  logPath: string,
  configPath: string,
): Record<string, string> {
  return {
    // The host resolves its project from PWD, not from the process cwd.
    PWD: sb.project,
    HOME: sb.home,
    XDG_CONFIG_HOME: join(sb.home, ".config"),
    XDG_DATA_HOME: join(sb.home, ".local/share"),
    XDG_STATE_HOME: join(sb.home, ".local/state"),
    XDG_CACHE_HOME: join(cacheRoot, "xdg"),
    npm_config_cache: join(cacheRoot, "npm"),
    TOOLU_PROBE_LOG: logPath,
    TOOLU_PROBE_CONFIG: configPath,
  };
}

const LogEntry = z.record(z.string(), z.unknown());

function readLog(path: string): Array<Record<string, unknown>> {
  if (!existsSync(path)) return [];
  return readFileSync(path, "utf8")
    .split("\n")
    .filter((line) => line !== "")
    .map((line) => LogEntry.parse(JSON.parse(line)));
}

export function openSession(cacheRoot: string, opts: SessionOptions = {}): ProbeSession {
  const sb = createSandbox({ git: true, files: opts.files ?? {} });
  const provider = startScriptedProvider(opts.scripts ?? {});
  const logPath = join(sb.root, "probe-log.jsonl");
  const configPath = join(sb.root, "probe-config.json");
  writeFileSync(configPath, JSON.stringify(opts.probeConfig?.(sb.root) ?? {}));
  const pluginsDir = join(sb.project, ".opencode/plugins");
  mkdirSync(pluginsDir, { recursive: true });
  for (const plugin of opts.localPlugins ?? [])
    copyFileSync(plugin, join(pluginsDir, basename(plugin)));
  writeFileSync(
    join(sb.project, "opencode.json"),
    `${JSON.stringify(hostConfig(provider.url, opts.model ?? "scripted", opts.config?.(sb.root) ?? {}), null, 2)}\n`,
  );
  return {
    sb,
    env: profileEnv(sb, cacheRoot, logPath, configPath),
    logPath,
    log: () => readLog(logPath),
    requests: () => provider.requests(),
    exists: (rel) => existsSync(join(sb.project, rel)),
    outside: (rel) => join(sb.root, rel),
    [Symbol.dispose]: () => {
      provider.stop();
      sb[Symbol.dispose]();
    },
  };
}
