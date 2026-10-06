/** OpenCode permission mapping and decision parity over shared JSON cases. */
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { z } from "zod";
import {
  applyDecisionToPermission,
  mapPermissionEventToTool,
  type PermissionEvaluationEvent,
} from "../permission-map.ts";

const EventSchema = z.strictObject({
  sessionID: z.string(),
  action: z.string(),
  resources: z.array(z.string()),
  effect: z.enum(["allow", "ask", "deny"]),
  metadata: z.record(z.string(), z.json()).optional(),
  message: z.string().optional(),
});
const DecisionSchema = z.discriminatedUnion("kind", [
  z.strictObject({ kind: z.literal("deny"), reason: z.string() }),
  z.strictObject({ kind: z.literal("ask"), reason: z.string() }),
  z.strictObject({ kind: z.literal("allow") }),
  z.strictObject({ kind: z.literal("advisory"), message: z.string() }),
  z.strictObject({ kind: z.literal("post_block"), reason: z.string() }),
  z.strictObject({
    kind: z.literal("runtime_failure"),
    reason: z.string(),
    code: z.enum(["timeout", "parse", "cancelled", "truncated", "spawn", "nonzero"]),
  }),
]);
const MappingSchema = z.discriminatedUnion("kind", [
  z.strictObject({ kind: z.literal("skip") }),
  z.strictObject({ kind: z.literal("deny"), reason: z.string() }),
  z.strictObject({
    kind: z.literal("request"),
    request: z.strictObject({
      session_id: z.string(),
      tool_use_id: z.string(),
      cwd: z.string(),
      tool_name: z.string(),
      tool_input: z.record(z.string(), z.json()),
    }),
  }),
]);
const MapSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("map"),
  event: EventSchema,
  expected: MappingSchema,
});
const DecisionCaseSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("decision"),
  checks: z
    .array(
      z.strictObject({
        decision: DecisionSchema,
        event: EventSchema,
        expected: z.strictObject({
          effect: z.enum(["allow", "ask", "deny"]),
          message: z.string().optional(),
        }),
      }),
    )
    .min(1),
});
const cases = readCaseFile(
  resolve(import.meta.dir, "../../../../../fixtures/opencode/permission-evaluate.json"),
);
const ctx = { cwd: "/proj", projectRoot: "/proj", worktree: "/proj", host: "opencode" };

function eventOf(value: z.infer<typeof EventSchema>): PermissionEvaluationEvent {
  return {
    sessionID: value.sessionID,
    action: value.action,
    resources: value.resources,
    effect: value.effect,
    ...(value.metadata === undefined ? {} : { metadata: value.metadata }),
    ...(value.message === undefined ? {} : { message: value.message }),
  };
}

for (const raw of cases) {
  if (raw.kind === "map") {
    const c = MapSchema.parse(raw);
    test(c.name, () => {
      expect(mapPermissionEventToTool(eventOf(c.event), ctx)).toEqual(c.expected);
    });
  } else if (raw.kind === "decision") {
    const c = DecisionCaseSchema.parse(raw);
    test(c.name, () => {
      for (const check of c.checks) {
        const event = eventOf(check.event);
        applyDecisionToPermission(check.decision, event);
        expect(event.effect).toBe(check.expected.effect);
        if (check.expected.message !== undefined)
          expect(event.message).toBe(check.expected.message);
        else expect(event.message).toBeUndefined();
      }
    });
  }
}
