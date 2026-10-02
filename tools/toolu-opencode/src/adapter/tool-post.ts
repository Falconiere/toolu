/** Completed OpenCode calls enter the same post-tool dispatcher as other hosts. */
import { join } from "node:path";
import type { Hooks } from "@opencode-ai/plugin";
import { dispatchPostTool, type ModuleResult } from "@toolu/core/dispatch";
import { gateStatusModule, pushWaiverModule } from "@toolu/core/gates";
import { gateEnv, type PermissionEvaluateHandlerOptions } from "./evaluate.ts";
import { mapToolCall, mcpServerNames, type ToolCall } from "./tool-before.ts";
import type { ToolAdviceStore } from "./tool-advice.ts";

type ToolAfter = NonNullable<Hooks["tool.execute.after"]>;
type AfterOutput = Parameters<ToolAfter>[1];
type Seen = { tool: string; expiresAt: number };

export type ToolPostHandler = {
  after: ToolAfter;
  begin(call: ToolCall): void;
  clear(): void;
};

const TTL_MS = 5 * 60_000;
const MAX_SEEN = 256;
const MAX_DIAGNOSTIC = 8192;

function key(call: ToolCall): string {
  return JSON.stringify([call.sessionID, call.callID]);
}

function object(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? Object.fromEntries(Object.entries(value))
    : undefined;
}

function field(value: Record<string, unknown> | undefined, name: string): string | undefined {
  const found = value?.[name];
  return typeof found === "string" && found.trim() !== "" ? found.trim() : undefined;
}

function bounded(message: string): string {
  return message.length <= MAX_DIAGNOSTIC
    ? message
    : `${message.slice(0, MAX_DIAGNOSTIC)}\n[toolu post-check diagnostic truncated]`;
}

/** The host has already executed the tool. A post block is diagnostic, not rollback. */
function append(output: AfterOutput, message: string): void {
  output.output = `${output.output}\n\n[toolu post-check after execution]\n${bounded(message)}`;
}

function resultMessage(result: ModuleResult): string | undefined {
  const stderr = result.stderr.trim();
  if (result.exitCode !== 0) {
    return `Post-tool checks failed to run (exit ${result.exitCode}): ${stderr || "no detail"}`;
  }
  const text = result.stdout.trim();
  if (text === "") return stderr === "" ? undefined : `Post-tool check warning: ${stderr}`;
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return `Post-tool checks returned invalid JSON${stderr === "" ? "" : `: ${stderr}`}`;
  }
  const doc = object(parsed);
  if (doc === undefined) return "Post-tool checks returned invalid output";
  const block = field(doc, "decision");
  if (block === "block") {
    return `Post-tool check failed: ${field(doc, "reason") ?? "check blocked"}`;
  }
  if (block !== undefined) return `Post-tool checks returned unknown decision: ${block}`;
  const hook = object(doc.hookSpecificOutput);
  const messages = [field(hook, "additionalContext"), field(doc, "systemMessage"), stderr].filter(
    (part) => part !== undefined && part !== "",
  );
  return messages.length === 0
    ? "Post-tool checks returned an empty decision"
    : messages.join("\n\n");
}

function interrupted(metadata: unknown): boolean {
  const doc = object(metadata);
  return doc?.interrupted === true || doc?.aborted === true || doc?.cancelled === true;
}

/** A shell result must prove its exit before it can clear quality state or promote a waiver. */
function shellOutcome(metadata: unknown): { exit: number } | { reason: string } {
  const doc = object(metadata);
  if (interrupted(metadata)) {
    return { reason: "shell call was interrupted; quality state was left unchanged" };
  }
  const exit: unknown = doc?.exit;
  if (typeof exit !== "number" || !Number.isInteger(exit) || exit < 0) {
    return { reason: "shell exit status was not confirmed; quality state was left unchanged" };
  }
  return { exit };
}

type PostContext = {
  opts: PermissionEvaluateHandlerOptions;
  servers: string[];
  pluginRoot: string;
  env: Record<string, string>;
};

async function dispatchMessage(
  input: Parameters<ToolAfter>[0],
  output: AfterOutput,
  context: PostContext,
): Promise<string | undefined> {
  const mapped = mapToolCall(input, input.args, context.opts.permissionContext, context.servers);
  if (mapped.kind === "skip") return undefined;
  if (mapped.kind === "deny") {
    return `Post-tool checks could not map this completed call: ${mapped.reason}`;
  }
  const request = mapped.request;
  const shell = request.tool_name === "Bash";
  const metadata: unknown = output.metadata;
  if (!shell && interrupted(metadata)) {
    return "tool call was interrupted; post-edit quality state was left unchanged";
  }
  const outcome = shell ? shellOutcome(metadata) : undefined;
  if (outcome !== undefined && "reason" in outcome) return outcome.reason;
  const payload = {
    ...request,
    tool_output: output.output,
    tool_response: {
      metadata: shell && outcome !== undefined ? { exit_code: outcome.exit } : metadata,
      interrupted: false,
    },
  };
  const result = await dispatchPostTool(JSON.stringify(payload), {
    builtins: [gateStatusModule, pushWaiverModule],
    libDir: join(context.pluginRoot, "hooks", "lib"),
    cwd: typeof request.cwd === "string" ? request.cwd : context.opts.permissionContext.cwd,
    env: context.env,
    ...(context.opts.selectedPluginSpecs === undefined
      ? {}
      : { selectedRegistrySpecs: context.opts.selectedPluginSpecs }),
  });
  return resultMessage(result);
}

/** Every after call is matched to its before call; a duplicate cannot re-run post checks. */
export function createToolPostHandler(
  opts: PermissionEvaluateHandlerOptions,
  advice: ToolAdviceStore,
): ToolPostHandler {
  const seen = new Map<string, Seen>();
  const servers = mcpServerNames(opts.permissionContext.projectRoot);
  const pluginRoot = join(opts.repoRoot, "plugins", "toolu");
  const env = gateEnv(opts, pluginRoot);
  const context = { opts, servers, pluginRoot, env };

  function prune(): void {
    const now = Date.now();
    for (const [id, entry] of seen) if (entry.expiresAt <= now) seen.delete(id);
  }

  const begin = (call: ToolCall): void => {
    prune();
    seen.delete(key(call));
  };

  const after: ToolAfter = async (input, output) => {
    prune();
    const id = key(input);
    if (seen.get(id)?.tool === input.tool) return;
    if (seen.size >= MAX_SEEN) {
      const oldest = seen.keys().next().value;
      if (oldest !== undefined) seen.delete(oldest);
    }
    seen.set(id, { tool: input.tool, expiresAt: Date.now() + TTL_MS });
    if (typeof output.output !== "string") {
      throw new Error("toolu: post-check delivery failed after execution: output is not text");
    }
    try {
      const message = await dispatchMessage(input, output, context);
      if (message !== undefined) append(output, message);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      append(output, `Post-tool checks failed to run: ${reason}`);
    }
    await advice.after(input, output);
  };

  return { after, begin, clear: () => seen.clear() };
}
