/** Jev readiness through real published wrappers and shared statusline cases. */
import { expect, test } from "bun:test";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import type { EnvPatch } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
import {
  applyFixtureActions,
  fixturePayload,
  fixtureString,
  FixtureActionSchema,
  FixturePayloadSchema,
} from "./fixture-actions.ts";
import { plain, render, report } from "./harness.ts";

const CheckSchema = z.strictObject({
  op: z.enum(["ends", "contains", "absent", "equal"]),
  value: z.json(),
});
const EnvSchema = z.record(z.string(), z.json());
const RunSchema = z.discriminatedUnion("op", [
  z.strictObject({
    op: z.literal("render-workspace"),
    env: EnvSchema.optional(),
    checks: z.array(CheckSchema).min(1),
  }),
  z.strictObject({
    op: z.literal("report"),
    dir: z.json().nullable(),
    cwd: z.json().optional(),
    env: EnvSchema.optional(),
    checks: z.array(CheckSchema).min(1),
  }),
  z.strictObject({
    op: z.literal("render"),
    payload: FixturePayloadSchema,
    plain: z.boolean().optional(),
    cwd: z.json().optional(),
    env: EnvSchema.optional(),
    checks: z.array(CheckSchema).min(1),
  }),
]);
const CaseSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("jev"),
  steps: z.array(z.union([FixtureActionSchema, RunSchema])).min(1),
});
const cases = readCaseFile(
  resolve(import.meta.dir, "../../../../../fixtures/statusline/cases.json"),
)
  .filter((raw) => raw.kind === "jev")
  .map((raw) => CaseSchema.parse(raw));

function envPatch(sb: Sandbox, raw: z.infer<typeof EnvSchema> | undefined): EnvPatch {
  return Object.fromEntries(
    Object.entries(raw ?? {}).map(([key, value]) => [
      key,
      value === null ? undefined : fixtureString(sb, value),
    ]),
  );
}

function assertChecks(
  sb: Sandbox,
  out: string,
  checks: readonly z.infer<typeof CheckSchema>[],
): void {
  for (const check of checks) {
    const value = fixtureString(sb, check.value);
    if (check.op === "ends") expect(out).toEndWith(value);
    else if (check.op === "contains") expect(out).toContain(value);
    else if (check.op === "absent") expect(out).not.toContain(value);
    else expect(out).toBe(value);
  }
}

for (const c of cases) {
  test.concurrent(c.name, async () => {
    using sb = createSandbox();
    for (const step of c.steps) {
      if (!("checks" in step)) {
        await applyFixtureActions(sb, [step]);
      } else if (step.op === "render-workspace") {
        mkdirSync(sb.path("workspace"), { recursive: true });
        const out = await render(sb, {
          payload: JSON.stringify({ workspace: { current_dir: sb.path("workspace") } }),
          env: envPatch(sb, step.env),
        });
        assertChecks(sb, out, step.checks);
      } else if (step.op === "report") {
        const dir = step.dir === null ? undefined : fixtureString(sb, step.dir);
        const cwd = step.cwd === undefined ? sb.root : fixtureString(sb, step.cwd);
        const out = await report(sb, dir, envPatch(sb, step.env), cwd);
        assertChecks(sb, out, step.checks);
      } else {
        const body = fixturePayload(sb, step.payload);
        const out = await render(sb, {
          ...(body === undefined ? {} : { payload: body }),
          ...(step.cwd === undefined ? {} : { cwd: fixtureString(sb, step.cwd) }),
          env: envPatch(sb, step.env),
        });
        assertChecks(sb, step.plain ? plain(out) : out, step.checks);
      }
    }
  });
}
