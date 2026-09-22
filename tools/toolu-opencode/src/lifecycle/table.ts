/**
 * OpenCode lifecycle handler map (stubs until #204 SDK wiring).
 * Aligns with portable-core / #209: filesystem bootstrap only.
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
  "session/resume": "deferred",
  "session/clear": "deferred",
  "session/unload": "deferred",
  "session/load": "deferred",
  prompt: "unsupported",
  pre_compact: "unsupported",
  "permission/evaluate": "deferred",
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
