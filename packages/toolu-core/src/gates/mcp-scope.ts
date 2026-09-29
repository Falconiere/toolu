/**
 * Whether an MCP call is mcp-blocker's business (#260), answered without zod:
 * the standalone `mcp__` hook asks this on every MCP call and loads the gate
 * (config, schemas) only when the answer is yes.
 */
import { join } from "node:path";
import { configExists, isFile } from "../config/config-files.ts";
import { SETTINGS_FILES } from "../config/settings-files.ts";
import { settingsDir } from "../config/settings-dir.ts";
import type { HostEnv, HostName } from "../host/host-name.ts";

/**
 * The server of an `mcp__<server>__<tool>` name (`case mcp__*__*`, then
 * `${rest%%__*}`), or undefined for any other tool.
 */
export function mcpServer(toolName: string): string | undefined {
  if (!toolName.startsWith("mcp__")) return undefined;
  const rest = toolName.slice("mcp__".length);
  const end = rest.indexOf("__");
  return end === -1 ? undefined : rest.slice(0, end);
}

export type McpScope = { env: HostEnv; host: HostName; pluginRoot?: string | undefined };

/** `<settings dir>/mcp-blocklist.txt`, when a settings directory resolves. */
export function mcpBlocklistFile(scope: McpScope): string | undefined {
  const dir = settingsDir({
    env: scope.env,
    ...(scope.pluginRoot === undefined ? {} : { pluginRoot: scope.pluginRoot }),
  });
  return dir === undefined ? undefined : join(dir, SETTINGS_FILES.mcpBlocklist);
}

/** Is there a blocklist file or a toolu config to consult? */
export function mcpHasSources(scope: McpScope): boolean {
  const list = mcpBlocklistFile(scope);
  return (list !== undefined && isFile(list)) || configExists({ env: scope.env, host: scope.host });
}
