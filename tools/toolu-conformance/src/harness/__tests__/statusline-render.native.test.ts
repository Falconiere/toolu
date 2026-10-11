/** Claude statusline bundle over shared real-repository states and JSON payloads. */
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import {
  applyFixtureActions,
  fixturePayload,
  fixtureString,
  FixtureActionSchema,
  FixturePayloadSchema,
} from "../statusline-fixture-actions.ts";
import { plain, render, runInOrder } from "../statusline-harness.ts";

const JEV_SUFFIX = "\x1b[2m | \x1b[0m\x1b[1m\x1b[32m[JEV:READY]\x1b[0m";

const CaseSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("render"),
  actions: z.array(FixtureActionSchema),
  runs: z
    .array(
      z.strictObject({
        payload: FixturePayloadSchema.optional(),
        plain: z.boolean(),
        env: z.record(z.string(), z.string().nullable()).optional(),
        checks: z
          .array(
            z.strictObject({
              op: z.enum(["contains", "absent", "equal", "ends", "not-match"]),
              value: z.json(),
            }),
          )
          .min(1),
      }),
    )
    .min(1),
});
const cases = readCaseFile(
  resolve(import.meta.dir, "../../../../../fixtures/statusline/cases.json"),
)
  .filter((raw) => raw.kind === "render")
  .map((raw) => CaseSchema.parse(raw));

for (const c of cases) {
  test.concurrent(c.name, async () => {
    using sb = createSandbox();
    await applyFixtureActions(sb, c.actions);
    await runInOrder(c.runs, async (run) => {
      const body = fixturePayload(sb, run.payload);
      const env =
        run.env === undefined
          ? undefined
          : Object.fromEntries(
              Object.entries(run.env).map(([key, value]) => [key, value ?? undefined]),
            );
      const raw = await render(sb, {
        ...(body === undefined ? {} : { payload: body }),
        ...(env === undefined ? {} : { env }),
      });
      expect(raw).toEndWith(JEV_SUFFIX);
      // #431 changes only Jev readiness; the shared renderer cases remain the
      // byte oracle for every other segment. The all-segments case checks Jev too.
      const legacy =
        c.name === "statusline: every segment in order, colours included"
          ? raw
          : raw.slice(0, -JEV_SUFFIX.length);
      const out = run.plain ? plain(legacy) : legacy;
      for (const check of run.checks) {
        const value = fixtureString(sb, check.value);
        if (check.op === "contains") expect(out).toContain(value);
        else if (check.op === "absent") expect(out).not.toContain(value);
        else if (check.op === "equal") expect(out).toBe(value);
        else if (check.op === "ends") expect(out).toEndWith(value);
        else expect(out).not.toMatch(new RegExp(value));
      }
    });
  });
}
