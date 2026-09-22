/** Normalized host event vocabulary (portable-core #210). */
import { z } from "zod";

const SessionContextSchema = z.object({
  sessionId: z.string().min(1),
  cwd: z.string().min(1),
  projectRoot: z.string().min(1),
  worktree: z.string().min(1),
});

const ToolContextSchema = SessionContextSchema.extend({
  toolCallId: z.string().min(1),
  toolName: z.string().min(1),
  toolInput: z.record(z.string(), z.unknown()).default({}),
});

export const NormalizedEventSchema = z.discriminatedUnion("type", [
  SessionContextSchema.extend({ type: z.literal("session/start") }),
  SessionContextSchema.extend({ type: z.literal("session/resume") }),
  SessionContextSchema.extend({ type: z.literal("session/clear") }),
  SessionContextSchema.extend({ type: z.literal("session/unload") }),
  SessionContextSchema.extend({
    type: z.literal("prompt"),
    prompt: z.string(),
  }),
  SessionContextSchema.extend({ type: z.literal("pre_compact") }),
  SessionContextSchema.extend({ type: z.literal("compaction") }),
  SessionContextSchema.extend({
    type: z.literal("permission/evaluate"),
    permission: z.string().min(1),
  }),
  ToolContextSchema.extend({ type: z.literal("tool/pre") }),
  ToolContextSchema.extend({
    type: z.literal("tool/post"),
    toolOutput: z.unknown().optional(),
  }),
  ToolContextSchema.extend({
    type: z.literal("shell/pre"),
    command: z.string().min(1),
  }),
]);

export type NormalizedEvent = z.infer<typeof NormalizedEventSchema>;

/** Bridge-facing event slug (subset used in BridgeRequest.event). */
export const BridgeEventSchema = z.enum([
  "tool/pre",
  "tool/post",
  "shell/pre",
  "session/start",
  "session/resume",
  "session/clear",
  "session/unload",
  "prompt",
  "pre_compact",
  "compaction",
  "permission/evaluate",
]);

export type BridgeEvent = z.infer<typeof BridgeEventSchema>;

/** Parse a normalized event envelope. */
export function parseNormalizedEvent(input: unknown): NormalizedEvent {
  return NormalizedEventSchema.parse(input);
}
