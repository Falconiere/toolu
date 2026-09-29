/**
 * The standalone `mcp__` PreToolUse hook (#260), as `mcp-blocker.sh` ran it:
 * one hook payload in, the mcp-blocker decision encoded for the host out. A
 * payload that is not a JSON object, a tool outside `mcp__*__*`, or a call
 * with no blocklist and no config is allowed before the gate, and with it the
 * config schemas, is even loaded. Config warnings go to stderr, as bash
 * printed them.
 */
import { encodeDecision } from "../host/host-encode.ts";
import { detectHost } from "../host/host-detect.ts";
import type { HostEnv } from "../host/host-name.ts";
import { configRoot, projectRoot } from "../host/host-roots.ts";
import { mcpHasSources, mcpServer } from "./mcp-scope.ts";

export type McpHookResult = { stdout: string; stderr: string; exitCode: number };

type JsonObject = Record<string, unknown>;

function isObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function parsed(stdin: string): JsonObject | undefined {
  try {
    const doc: unknown = JSON.parse(stdin);
    return isObject(doc) ? doc : undefined;
  } catch {
    return undefined;
  }
}

function field(doc: JsonObject, key: string, fallback: string): string {
  const value = doc[key];
  return typeof value === "string" && value !== "" ? value : fallback;
}

const SILENT: McpHookResult = { stdout: "", stderr: "", exitCode: 0 };

export async function mcpHook(
  stdin: string,
  options: { pluginRoot: string; env?: HostEnv },
): Promise<McpHookResult> {
  const doc = parsed(stdin);
  const toolName = doc === undefined ? "" : field(doc, "tool_name", "");
  if (doc === undefined || mcpServer(toolName) === undefined) return SILENT;
  const hostEnv = options.env ?? process.env;
  const host = detectHost({ env: hostEnv });
  // Resolve the project once (a git spawn when the host sets no project
  // variable) and pin it, so the config lookups below do not spawn again.
  const project = projectRoot({ env: hostEnv, host });
  const env = project === undefined ? hostEnv : { ...hostEnv, TOOLU_PROJECT_DIR: project };
  if (!mcpHasSources({ env, host, pluginRoot: options.pluginRoot })) return SILENT;
  const { mcpBlockerModule } = await import("./mcp-blocker.ts");
  const root = project ?? process.cwd();
  const event = {
    type: "tool/pre" as const,
    sessionId: field(doc, "session_id", "unknown"),
    cwd: field(doc, "cwd", root),
    projectRoot: root,
    worktree: root,
    toolCallId: field(doc, "tool_use_id", "unknown"),
    toolName,
    toolInput: isObject(doc.tool_input) ? doc.tool_input : {},
  };
  const warnings: string[] = [];
  const gate = mcpBlockerModule({
    pluginRoot: options.pluginRoot,
    warn: (line) => warnings.push(`toolu-config: ${line}\n`),
  });
  const ctx = { host, env, configRoot: configRoot({ env, host }), projectRoot: root, raw: doc };
  const decision = await gate.run(event, ctx);
  const out = encodeDecision(host === "codex" ? "codex" : "claude", "tool/pre", decision);
  return {
    stdout: out.kind === "command" ? out.stdout : "",
    stderr: warnings.join(""),
    exitCode: 0,
  };
}
