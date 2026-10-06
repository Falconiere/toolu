/** Strict schemas for the shared ast-grep golden records (#268). */
import { resolve } from "node:path";
import { ActionSchema, readCaseFile } from "@toolu/conformance/harness/json-cases";
import { z } from "zod";

const Hosts = z.array(z.enum(["claude", "codex"])).optional();
const DeviationSchema = z.union([
  z.strictObject({ silent: z.literal(true) }),
  z.strictObject({ contains: z.string(), excludes: z.string().optional() }),
  z.strictObject({ contains: z.string().optional(), excludes: z.string() }),
]);
const NudgeSchema = z.strictObject({
  name: z.string().min(1),
  hosts: Hosts,
  state: z.enum(["available", "missing", "opt-out"]).optional(),
  toolName: z.string(),
  toolInput: z.record(z.string(), z.json()),
  deviation: DeviationSchema.optional(),
});
const SavingsSchema = z.strictObject({
  name: z.string().min(1),
  hosts: Hosts,
  payload: z.record(z.string(), z.json()),
  env: z
    .record(z.string(), z.union([z.string(), z.strictObject({ $path: z.string() })]))
    .optional(),
  setup: z.array(ActionSchema),
  payloadSetup: z.array(ActionSchema),
  deviation: DeviationSchema.optional(),
});
const ReportSchema = z.strictObject({
  name: z.string().min(1),
  ledger: z.string().optional(),
  noArgument: z.boolean().optional(),
  deviation: z.string().optional(),
});

export type NudgeCase = z.infer<typeof NudgeSchema>;
export type SavingsCase = z.infer<typeof SavingsSchema>;
export type ReportCase = z.infer<typeof ReportSchema>;
export type Deviation = z.infer<typeof DeviationSchema>;

const ROOT = resolve(import.meta.dir, "../../../../../fixtures/ast-grep");
export const NUDGE_CASES: readonly NudgeCase[] = readCaseFile(resolve(ROOT, "nudge.json")).map(
  (item) => NudgeSchema.parse(item),
);
export const SAVINGS_CASES: readonly SavingsCase[] = readCaseFile(
  resolve(ROOT, "savings.json"),
).map((item) => SavingsSchema.parse(item));
export const REPORT_CASES: readonly ReportCase[] = readCaseFile(resolve(ROOT, "report.json")).map(
  (item) => ReportSchema.parse(item),
);
