/** Strict v1 state schemas against the shared positive and negative records. */
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { z } from "zod";
import {
  EditRecordSchema,
  GateFileSchema,
  TELEMETRY_EXTRAS,
  TelemetryLineSchema,
} from "../state-schema.ts";

const SchemaCase = z.strictObject({
  name: z.string(),
  kind: z.literal("schema"),
  schema: z.enum(["gate", "telemetry", "edit"]),
  input: z.json(),
  valid: z.boolean(),
});
const EventCase = z.strictObject({
  name: z.string(),
  kind: z.literal("telemetry-events"),
  lines: z.array(z.json()),
  events: z.array(z.string()),
});
const ExtrasCase = z.strictObject({
  name: z.string(),
  kind: z.literal("telemetry-extras"),
  checks: z.array(
    z.strictObject({
      event: z.enum(["docs_nudge", "gate_fail"]),
      input: z.json(),
    }),
  ),
});
const cases = readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/state/cases.json"));

for (const raw of cases) {
  if (raw.kind === "schema") {
    const c = SchemaCase.parse(raw);
    test(c.name, () => {
      const schema =
        c.schema === "gate"
          ? GateFileSchema
          : c.schema === "telemetry"
            ? TelemetryLineSchema
            : EditRecordSchema;
      expect(schema.safeParse(c.input).success).toBe(c.valid);
    });
  } else if (raw.kind === "telemetry-events") {
    const c = EventCase.parse(raw);
    test(c.name, () => {
      for (const line of c.lines) expect(TelemetryLineSchema.safeParse(line).success).toBe(true);
      expect(
        c.lines
          .map((line) => z.strictObject({ event: z.string() }).passthrough().parse(line).event)
          .sort(),
      ).toEqual(c.events);
      expect(Object.keys(TELEMETRY_EXTRAS).sort()).toEqual(c.events);
    });
  } else if (raw.kind === "telemetry-extras") {
    const c = ExtrasCase.parse(raw);
    test(c.name, () => {
      for (const check of c.checks) {
        expect(TELEMETRY_EXTRAS[check.event].safeParse(check.input).success).toBe(false);
      }
    });
  }
}
