/**
 * OpenCode lifecycle handler map.
 * Prompt and compaction context are delivered on the pinned hooks (#341).
 * Resume, clear and load have no event on this pin. Post-tool checks use
 * tool.execute.after; the remaining deferred entries keep their own scope.
 */

export type LifecycleSupport = "supported" | "deferred" | "unsupported";

export type LifecycleEvent =
  | "session/start"
  | "session/resume"
  | "session/clear"
  | "session/unload"
  | "session/load"
  | "prompt"
  | "pre_compact"
  | "permission/evaluate"
  | "tool/pre"
  | "tool/post"
  | "shell/pre";

const TABLE: Record<LifecycleEvent, LifecycleSupport> = {
  "session/start": "supported",
  "session/resume": "unsupported",
  "session/clear": "unsupported",
  "session/unload": "supported",
  "session/load": "unsupported",
  prompt: "supported",
  pre_compact: "supported",
  "permission/evaluate": "supported",
  "tool/pre": "deferred",
  "tool/post": "supported",
  "shell/pre": "deferred",
};

export function lifecycleSupport(event: LifecycleEvent): LifecycleSupport {
  return TABLE[event];
}

export function lifecycleSupportTable(): Record<LifecycleEvent, LifecycleSupport> {
  return { ...TABLE };
}
