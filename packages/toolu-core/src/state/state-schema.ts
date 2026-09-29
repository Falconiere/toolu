/**
 * Persisted-state contracts (#255): the gate file, telemetry lines and edit
 * records, strict v1. The gate file carries no `version` field, because v1 is
 * the implicit format bash writes and TypeScript must write the same bytes. An
 * explicit `version: 1` is accepted; any other version fails the schema.
 */
import { z } from "zod";

export const GATE_FILE_VERSION = 1;
export const TELEMETRY_VERSION = 1;

const Version = z.literal(GATE_FILE_VERSION).optional();

export const GateEntrySchema = z.strictObject({
  source: z.string(),
  reason: z.string(),
  violations: z.string(),
  updatedAt: z.string(),
});

const PassingSchema = z.strictObject({
  version: Version,
  status: z.literal("passing"),
  source: z.string(),
  updatedAt: z.string(),
});

const FailingSchema = z.strictObject({
  version: Version,
  status: z.literal("failing"),
  reason: z.string(),
  source: z.string(),
  file: z.string(),
  violations: z.string(),
  entries: z.record(z.string(), GateEntrySchema).optional(),
  updatedAt: z.string(),
});

export const GateFileSchema = z.discriminatedUnion("status", [PassingSchema, FailingSchema]);

export type GateEntry = z.infer<typeof GateEntrySchema>;
export type GateFile = z.infer<typeof GateFileSchema>;
export type GateEntries = Record<string, GateEntry>;

const text = z.string();
const maybeText = z.string().nullable();

/**
 * Every event a caller records, with its exact extras. Closed on purpose:
 * telemetry never takes a command line, a tool payload or a nested value, so
 * a secret cannot reach the log through an unexpected field.
 */
export const TELEMETRY_EXTRAS = {
  gate_fail: z.strictObject({ file: text, source: text }),
  gate_clear: z.strictObject({ file: text, source: text }),
  step_run: z.strictObject({
    step_id: text,
    status: text,
    exit_code: z.number(),
    duration_s: z.number(),
    attempt: z.number(),
  }),
  ac_coverage: z.strictObject({ covered: z.number(), uncovered: z.number() }),
  docs_attested: z.strictObject({ decision: text }),
  docs_nudge: z.strictObject({}),
  push_check: z.strictObject({ result: text, reason_code: text, round: z.number().nullable() }),
  delegation: z.strictObject({
    model: maybeText,
    subagent_type: maybeText,
    reasoning_effort: maybeText,
    step_id: maybeText,
    step_model: maybeText,
  }),
} as const;

export type TelemetryEvent = keyof typeof TELEMETRY_EXTRAS;
export type TelemetryExtras = { [E in TelemetryEvent]: z.infer<(typeof TELEMETRY_EXTRAS)[E]> };

export function isTelemetryEvent(event: string): event is TelemetryEvent {
  return Object.hasOwn(TELEMETRY_EXTRAS, event);
}

const Protocol = {
  v: z.literal(TELEMETRY_VERSION),
  t: text,
  branch: text,
};

/** One persisted telemetry line: the event's extras plus the protocol fields. */
export const TelemetryLineSchema = z.discriminatedUnion("event", [
  TELEMETRY_EXTRAS.gate_fail.extend({ ...Protocol, event: z.literal("gate_fail") }),
  TELEMETRY_EXTRAS.gate_clear.extend({ ...Protocol, event: z.literal("gate_clear") }),
  TELEMETRY_EXTRAS.step_run.extend({ ...Protocol, event: z.literal("step_run") }),
  TELEMETRY_EXTRAS.ac_coverage.extend({ ...Protocol, event: z.literal("ac_coverage") }),
  TELEMETRY_EXTRAS.docs_attested.extend({ ...Protocol, event: z.literal("docs_attested") }),
  TELEMETRY_EXTRAS.docs_nudge.extend({ ...Protocol, event: z.literal("docs_nudge") }),
  TELEMETRY_EXTRAS.push_check.extend({ ...Protocol, event: z.literal("push_check") }),
  TELEMETRY_EXTRAS.delegation.extend({ ...Protocol, event: z.literal("delegation") }),
]);

export type TelemetryLine = z.infer<typeof TelemetryLineSchema>;

export const EDIT_OPERATIONS = ["add", "update", "delete", "write", "move"] as const;

/** One affected path: a move yields an `update` with `moved_to` and a `move` with `from`. */
export const EditRecordSchema = z.strictObject({
  path: z.string().min(1),
  operation: z.enum(EDIT_OPERATIONS),
  moved_to: z.string().optional(),
  from: z.string().optional(),
});

export type EditRecord = z.infer<typeof EditRecordSchema>;
