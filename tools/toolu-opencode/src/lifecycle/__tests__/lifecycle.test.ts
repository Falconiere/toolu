/** OpenCode lifecycle event support from shared parity records and the pinned SDK. */
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import type { Hooks } from "@opencode-ai/plugin";
import { z } from "zod";
import { lifecycleSupport } from "../table.ts";

const EventSchema = z.enum([
  "session/start",
  "session/resume",
  "session/clear",
  "session/unload",
  "session/load",
  "prompt",
  "pre_compact",
  "permission/evaluate",
  "tool/pre",
  "tool/post",
  "shell/pre",
]);
const CaseSchema = z.strictObject({
  name: z.string(),
  events: z.partialRecord(EventSchema, z.enum(["supported", "deferred", "unsupported"])),
  sdkHooks: z.partialRecord(z.enum(["prompt", "pre_compact", "startup"]), z.string()).optional(),
});

/** A renamed SDK hook fails typecheck while the JSON checks its published value. */
const CONTEXT_HOOKS = {
  startup: "experimental.chat.system.transform",
  prompt: "chat.message",
  pre_compact: "experimental.session.compacting",
} as const satisfies Record<string, keyof Hooks>;

const cases = readCaseFile(
  resolve(import.meta.dir, "../../../../../fixtures/opencode/lifecycle-events.json"),
).map((raw) => CaseSchema.parse(raw));
for (const c of cases) {
  test(c.name, () => {
    for (const [event, support] of Object.entries(c.events)) {
      expect(lifecycleSupport(EventSchema.parse(event))).toBe(support);
    }
    if (c.sdkHooks !== undefined) {
      for (const [name, hook] of Object.entries(c.sdkHooks)) {
        expect(hook).toBe(CONTEXT_HOOKS[z.enum(["prompt", "pre_compact", "startup"]).parse(name)]);
      }
    }
  });
}
