/** A post-tool event and registry context over a real sandbox, as the dispatcher builds them. */
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import type { RegistryContext, RegistryHookEvent } from "../../registry/registry-types.ts";

export type Call = {
  toolName?: string | undefined;
  input?: Record<string, unknown> | undefined;
  env?: Record<string, string> | undefined;
  edit?: RegistryContext["edit"] | undefined;
};

export function postEvent(sb: Sandbox, call: Call): RegistryHookEvent {
  return {
    type: "tool/post",
    sessionId: "s",
    cwd: sb.project,
    projectRoot: sb.project,
    worktree: sb.project,
    toolCallId: "t",
    toolName: call.toolName ?? "Write",
    toolInput: call.input ?? {},
  };
}

export function postContext(sb: Sandbox, call: Call): RegistryContext {
  return {
    host: "claude",
    env: { HOME: sb.home, PATH: process.env.PATH ?? "/usr/bin:/bin", ...call.env },
    configRoot: sb.configDir("claude", "user"),
    projectRoot: sb.project,
    cwd: sb.project,
    raw: { tool_name: call.toolName ?? "Write", tool_input: call.input ?? {} },
    ...(call.edit === undefined ? {} : { edit: call.edit }),
  };
}
