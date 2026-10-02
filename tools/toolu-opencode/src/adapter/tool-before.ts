/**
 * OpenCode `tool.execute.before` → toolu's native pre-tool gates (#336).
 *
 * A throw is the host's only refusal (probes `deny.*`): it stops the call before
 * any side effect and becomes the tool error the model sees. The host has no ask
 * channel (`permission.ask-hook`), so every non-allow decision throws until OP-05
 * (#339) degrades `ask` by gate class. Tools beyond bash/edit/write pass through
 * until OP-03 (#337) normalizes them; the user's own permission rules still apply.
 */
import type { Hooks } from "@opencode-ai/plugin";
import { z } from "zod";
import { createGateDecider, type PermissionEvaluateHandlerOptions } from "./evaluate.ts";
import type { PermissionContext, PermissionMapping } from "./permission-map.ts";

export type ToolBefore = NonNullable<Hooks["tool.execute.before"]>;
export type ToolCall = { tool: string; sessionID: string; callID: string };

const ShellArgs = z.looseObject({ command: z.string().min(1) });
const FileArgs = z.looseObject({ filePath: z.string().min(1) });

function request(call: ToolCall, ctx: PermissionContext, toolName: string, input: object) {
  return {
    kind: "request" as const,
    request: {
      session_id: call.sessionID,
      tool_use_id: call.callID,
      cwd: ctx.cwd,
      tool_name: toolName,
      tool_input: input,
    },
  };
}

/** Map one host tool call (its validated `args`) to the core dispatcher's hook input. */
export function mapToolCall(
  call: ToolCall,
  args: unknown,
  ctx: PermissionContext,
): PermissionMapping {
  switch (call.tool) {
    case "bash": {
      const parsed = ShellArgs.safeParse(args);
      if (!parsed.success)
        return { kind: "deny", reason: "toolu: bash call missing a string command" };
      return request(call, ctx, "Bash", { command: parsed.data.command });
    }
    case "edit":
    case "write": {
      const parsed = FileArgs.safeParse(args);
      if (!parsed.success)
        return { kind: "deny", reason: `toolu: ${call.tool} call missing a string filePath` };
      const toolName = call.tool === "edit" ? "Edit" : "Write";
      return request(call, ctx, toolName, { file_path: parsed.data.filePath });
    }
    default:
      return { kind: "skip" };
  }
}

/** Refuse every tool call with `reason`: the fail-closed hook when toolu is not ready. */
export function createDenyAllToolBefore(reason: string): ToolBefore {
  return () => Promise.reject(new Error(reason));
}

/** Run the native gates for each covered tool call; throw on any decision but allow/advisory. */
export function createToolBeforeHandler(opts: PermissionEvaluateHandlerOptions): ToolBefore {
  const decider = createGateDecider(opts);
  if (!decider.ok) return createDenyAllToolBefore(decider.reason);
  return async (input, output) => {
    const args: unknown = output.args;
    const mapping = mapToolCall(input, args, opts.permissionContext);
    if (mapping.kind === "skip") return;
    if (mapping.kind === "deny") throw new Error(mapping.reason);
    const decision = await decider.decide(mapping.request);
    switch (decision.kind) {
      case "allow":
      case "advisory":
        return;
      case "deny":
      case "ask":
      case "post_block":
      case "runtime_failure":
        throw new Error(decision.reason);
    }
  };
}
