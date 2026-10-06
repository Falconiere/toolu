/** Shared JSON inputs for the quality runner's real sandbox tests (#459). */
import { resolve } from "node:path";
import {
  ActionSchema,
  materializeCaseValue,
  readCaseFile,
} from "@toolu/conformance/harness/json-cases";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";

const CallSchema = z.strictObject({
  toolName: z.string().optional(),
  input: z.record(z.string(), z.json()).optional(),
  env: z.record(z.string(), z.string()).optional(),
  edit: z
    .strictObject({
      operation: z.enum(["add", "update", "delete", "write", "move"]),
      from: z.string(),
      movedTo: z.string(),
    })
    .optional(),
});
const DecisionSchema = z.union([
  z.strictObject({ kind: z.literal("allow") }),
  z.strictObject({ kind: z.literal("advisory"), message: z.string() }),
]);
const RunSchema = z.strictObject({
  name: z.string().min(1),
  kind: z.literal("run"),
  files: z.record(z.string(), z.string()),
  steps: z.array(
    z.strictObject({
      errors: z.array(z.string()),
      advisories: z.array(z.string()).optional(),
      call: CallSchema,
      decision: DecisionSchema.optional(),
      entries: z.record(z.string(), z.string()).optional(),
      gateStatus: z.enum(["passing", "failing", "missing"]).optional(),
    }),
  ),
});
const EditCheckSchema = z.discriminatedUnion("subject", [
  z.strictObject({
    subject: z.literal("edited"),
    call: CallSchema,
    field: z.enum(["path", "full", "removed", "undefined"]),
    expected: z.json().optional(),
  }),
  z.strictObject({ subject: z.literal("regular"), path: z.string(), expected: z.boolean() }),
  z.strictObject({ subject: z.literal("linked"), path: z.json(), expected: z.boolean() }),
]);
const EditSchema = z.strictObject({
  name: z.string().min(1),
  kind: z.literal("edit"),
  setup: z.array(ActionSchema),
  checks: z.array(EditCheckSchema).min(1),
});
const ScanExpectSchema = z.discriminatedUnion("kind", [
  z.strictObject({
    kind: z.literal("rules"),
    empty: z.boolean(),
    count: z.number().int().nonnegative(),
    hitsByRule: z.record(
      z.string(),
      z.array(
        z.strictObject({
          ruleId: z.string(),
          line: z.number().int(),
          excerpt: z.string(),
          text: z.string(),
          first: z.boolean(),
        }),
      ),
    ),
  }),
  z.strictObject({
    kind: z.literal("lines"),
    lines: z.array(
      z.strictObject({
        line: z.number().int(),
        text: z.string(),
        first: z.boolean(),
      }),
    ),
  }),
  z.strictObject({
    kind: z.literal("failure"),
    stage: z.literal("ast-grep"),
    nonzeroExit: z.boolean(),
    stderrMin: z.number().int(),
    stderrMax: z.number().int(),
  }),
  z.strictObject({ kind: z.literal("exact"), result: z.json() }),
]);
const ScanSchema = z.strictObject({
  name: z.string().min(1),
  kind: z.literal("scan"),
  requiresAstGrep: z.boolean(),
  checks: z
    .array(
      z.strictObject({
        path: z.string(),
        body: z.string(),
        rules: z.string(),
        env: z
          .discriminatedUnion("kind", [
            z.strictObject({ kind: z.literal("stubAstGrep"), body: z.string() }),
            z.strictObject({ kind: z.literal("pathWithoutAstGrep") }),
          ])
          .optional(),
        expect: ScanExpectSchema,
      }),
    )
    .min(1),
});

export type RunnerCall = z.infer<typeof CallSchema>;
export type RunCase = z.infer<typeof RunSchema>;
export type EditCase = z.infer<typeof EditSchema>;
export type ScanCase = z.infer<typeof ScanSchema>;

export function materializeRunnerCall(sb: Sandbox, value: RunnerCall): RunnerCall {
  return CallSchema.parse(materializeCaseValue(sb, value));
}

const PATH = resolve(import.meta.dir, "../../../../../fixtures/quality/runner.json");
const CASES = readCaseFile(PATH);
export const RUN_CASES: readonly RunCase[] = CASES.filter((item) => item.kind === "run").map(
  (item) => RunSchema.parse(item),
);
export const EDIT_CASES: readonly EditCase[] = CASES.filter((item) => item.kind === "edit").map(
  (item) => EditSchema.parse(item),
);
export const SCAN_CASES: readonly ScanCase[] = CASES.filter((item) => item.kind === "scan").map(
  (item) => ScanSchema.parse(item),
);
