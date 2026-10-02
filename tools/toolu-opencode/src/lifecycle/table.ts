/**
 * OpenCode lifecycle handler map.
 * Prompt and compaction context are delivered on the pinned hooks (#341).
 * Resume, clear and load have no event on this pin. Tool and shell hooks
 * stay deferred for their own work package.
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
  "tool/post": "deferred",
  "shell/pre": "deferred",
};

export function lifecycleSupport(event: LifecycleEvent): LifecycleSupport {
  return TABLE[event];
}

export function lifecycleSupportTable(): Record<LifecycleEvent, LifecycleSupport> {
  return { ...TABLE };
}
