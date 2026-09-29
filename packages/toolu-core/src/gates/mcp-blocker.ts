/**
 * mcp-blocker (#260), the native port of `pre-tools/modules/mcp-blocker.sh`.
 * An `mcp__<server>__<tool>` call whose server starts with an entry of
 * `settings/mcp-blocklist.txt`, or is a `mcp.<server>: false` key of the toolu
 * config, reaches the user as `gates.mcpBlocker.mode` says (ask by default,
 * block where the host cannot prompt). An entry's ` -> <text>` hint names the
 * replacement.
 */
import { readFileSync } from "node:fs";
import { dirname } from "node:path";
import { isFile } from "../config/config-files.ts";
import { isJsonObject, type JsonObject, type LoadedConfig } from "../config/config-load.ts";
import { section } from "../config/config-read.ts";
import { gateDecision, gateMode, guardrailWarning, type GateMode } from "../config/gate-mode.ts";
import { mcpBlocklist, type McpBlockEntry } from "../config/settings.ts";
import type { Decision } from "../decision/decision.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { ALLOW, gateConfig, type GateModule, type GateModuleOptions } from "./gate-module.ts";
import { mcpBlocklistFile, mcpHasSources, mcpServer } from "./mcp-scope.ts";

/** A config key parsed like a blocklist line: `prefix[ -> redirect]`, trimmed. */
function configEntry(key: string): McpBlockEntry {
  const arrow = key.indexOf(" -> ");
  return {
    prefix: (arrow === -1 ? key : key.slice(0, arrow)).trim(),
    redirect: arrow === -1 ? "" : key.slice(arrow + 4),
  };
}

/**
 * The config's `mcp` section. When an envelope is invalid, `loadConfig` drops
 * the whole config (and gate modes block); the `mcp` keys are still read from
 * the files as bash read them, so a `mcp.<server>: false` keeps blocking.
 */
function mcpSection(config: LoadedConfig): JsonObject {
  if (config.invalid === undefined) return section(config, "mcp") ?? {};
  const merged: JsonObject = {};
  for (const path of [config.files.user, config.files.project]) {
    if (path === undefined || !isFile(path)) continue;
    try {
      const raw: unknown = JSON.parse(readFileSync(path, "utf8"));
      const mcp = isJsonObject(raw) ? raw.mcp : undefined;
      if (isJsonObject(mcp)) Object.assign(merged, mcp);
    } catch {
      // Malformed JSON is ignored, as `toolu_load_config` ignored it.
    }
  }
  return merged;
}

function matchEntry(entries: readonly McpBlockEntry[], server: string): McpBlockEntry | undefined {
  return entries.find((entry) => entry.prefix !== "" && server.startsWith(entry.prefix));
}

type Block = { tool: string; server: string; origin: string; redirect: string };

function reason(mode: GateMode, block: Block): string {
  const { tool, server, origin } = block;
  const headline = `Claude is calling MCP tool "${tool}", on the blocked server "${server}" (${origin}).`;
  let detail =
    "Blocked MCP servers are ones this project has decided not to reach through an MCP bridge — usually because a CLI path exists that is auditable and scoped, where the MCP tool is neither.";
  if (block.redirect !== "") detail = `${detail} Use instead: ${block.redirect}`;
  if (mode === "ask") return guardrailWarning(headline, detail);
  if (mode === "advise") {
    return `MCP server "${server}" is blocked (${origin}). ${detail} The call was NOT stopped — gates.mcpBlocker.mode is 'advise'.`;
  }
  return `MCP server "${server}" is blocked (${origin}). ${detail}`;
}

function decide(
  event: RegistryHookEvent,
  ctx: RegistryContext,
  options: GateModuleOptions,
): Decision {
  const tool = event.toolName;
  const server = mcpServer(tool);
  if (server === undefined) return ALLOW;
  const scope = { env: ctx.env, host: ctx.host, pluginRoot: options.pluginRoot };
  // Every MCP call routes here: with nothing to consult, skip loading config.
  if (!mcpHasSources(scope)) return ALLOW;
  const list = mcpBlocklistFile(scope);
  const fromFile = list === undefined ? undefined : matchEntry(mcpBlocklist(dirname(list)), server);
  const config = gateConfig(ctx, options);
  const disabled = Object.entries(mcpSection(config))
    .filter(([, value]) => value === false)
    .map(([key]) => configEntry(key));
  const fromConfig = fromFile === undefined ? matchEntry(disabled, server) : undefined;
  const match = fromFile ?? fromConfig;
  if (match === undefined) return ALLOW;
  const origin =
    fromFile === undefined
      ? `disabled in your toolu config (mcp.${server}=false)`
      : "listed in settings/mcp-blocklist.txt";
  const mode = gateMode(config, "mcpBlocker", { host: ctx.host, event: "tool/pre" });
  const block = { tool, server, origin, redirect: match.redirect };
  return gateDecision(mode, reason(mode, block)) ?? ALLOW;
}

/** The mcp-blocker built-in module. */
export function mcpBlockerModule(options: GateModuleOptions = {}): GateModule {
  return {
    kind: "native",
    name: "mcp-blocker",
    run: (event, ctx) => Promise.resolve(decide(event, ctx, options)),
  };
}
