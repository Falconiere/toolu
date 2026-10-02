import { expect, test } from "bun:test";
import type { Hooks } from "@opencode-ai/plugin";
import { lifecycleSupport } from "../table.ts";

/** Context hooks that exist on the pinned SDK. A rename in the SDK fails this file's typecheck. */
const CONTEXT_HOOKS = {
  startup: "experimental.chat.system.transform",
  prompt: "chat.message",
  pre_compact: "experimental.session.compacting",
} as const satisfies Record<string, keyof Hooks>;

test("session, prompt and compaction are supported on the pinned hooks", () => {
  expect(lifecycleSupport("session/start")).toBe("supported");
  expect(lifecycleSupport("prompt")).toBe("supported");
  expect(lifecycleSupport("pre_compact")).toBe("supported");
  expect(lifecycleSupport("session/unload")).toBe("supported");
  expect(lifecycleSupport("permission/evaluate")).toBe("supported");
  expect(CONTEXT_HOOKS.prompt).toBe("chat.message");
  expect(CONTEXT_HOOKS.pre_compact).toBe("experimental.session.compacting");
  expect(CONTEXT_HOOKS.startup).toBe("experimental.chat.system.transform");
});

test("resume, clear and load have no OpenCode event", () => {
  expect(lifecycleSupport("session/resume")).toBe("unsupported");
  expect(lifecycleSupport("session/clear")).toBe("unsupported");
  expect(lifecycleSupport("session/load")).toBe("unsupported");
});
