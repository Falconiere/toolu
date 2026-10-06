/** The shared gate-file byte golden (#415): every step's file, clear outcome and drop log, byte for byte. */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { clearGateFile, recordGateFailure } from "../gate-file.ts";

const ExpectSchema = z.strictObject({
  bytes: z.string().nullable(),
  outcome: z.enum(["cleared", "noop"]).optional(),
  dropLog: z.string().nullable(),
});
const StepSchema = z.discriminatedUnion("op", [
  z.strictObject({
    op: z.literal("record"),
    file: z.string(),
    source: z.string(),
    reason: z.string(),
    violations: z.string(),
    now: z.string(),
    expect: ExpectSchema,
  }),
  z.strictObject({
    op: z.literal("clear"),
    file: z.string(),
    source: z.string(),
    now: z.string(),
    expect: ExpectSchema,
  }),
]);
const CaseSchema = z.strictObject({
  name: z.string(),
  initial: z.string().nullable(),
  steps: z.array(StepSchema).min(1),
});
const cases = readCaseFile(
  resolve(import.meta.dir, "../../../../../fixtures/state/gate-bytes.json"),
).map((raw) => CaseSchema.parse(raw));

test("the golden holds its sixteen sequences", () => {
  expect(cases).toHaveLength(16);
});

for (const c of cases) {
  test(c.name, () => {
    using sb = createSandbox();
    const dir = join(sb.project, ".claude", "tmp");
    mkdirSync(dir, { recursive: true });
    const gate = join(dir, "quality-gate-status.json");
    if (c.initial !== null) writeFileSync(gate, c.initial);
    const env = { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project, TOOLU_HOST_OVERRIDE: "claude" };
    for (const step of c.steps) {
      const options = {
        env,
        host: "claude" as const,
        now: () => new Date(step.now),
        warn: () => {},
      };
      if (step.op === "record") {
        recordGateFailure(gate, step.file, step.source, step.reason, step.violations, options);
      } else {
        expect<string>(clearGateFile(gate, step.file, step.source, options)).toBe(
          z.string().parse(step.expect.outcome),
        );
      }
      expect(existsSync(gate) ? readFileSync(gate, "utf8") : null).toBe(step.expect.bytes);
      const log = `${gate}.dropped.log`;
      expect(existsSync(log) ? readFileSync(log, "utf8") : null).toBe(step.expect.dropLog);
    }
  });
}
