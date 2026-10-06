/** Codex and OpenCode status report states from shared fixture records. */
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { entryArgv } from "@toolu/conformance/harness/entry-command";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { applyFixtureActions, fixtureString, FixtureActionSchema } from "./fixture-actions.ts";
import { PLUGIN, report } from "./harness.ts";

const ReportSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("report"),
  actions: z.array(FixtureActionSchema),
  host: z.enum(["codex", "opencode"]),
  dir: z.json(),
  env: z.record(z.string(), z.string().nullable()).optional(),
  checks: z
    .array(z.strictObject({ op: z.enum(["equal", "contains", "absent"]), value: z.json() }))
    .min(1),
});
const ErrorSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("report-error"),
  overrides: z.array(z.string()).min(1),
  expectedPrefix: z.string(),
});
const cases = readCaseFile(
  resolve(import.meta.dir, "../../../../../fixtures/statusline/cases.json"),
);

for (const raw of cases) {
  if (raw.kind === "report") {
    const c = ReportSchema.parse(raw);
    test.concurrent(c.name, async () => {
      using sb = createSandbox();
      await applyFixtureActions(sb, c.actions);
      const env = {
        ...Object.fromEntries(
          Object.entries(c.env ?? {}).map(([key, value]) => [key, value ?? undefined]),
        ),
        TOOLU_HOST_OVERRIDE: c.host,
      };
      const out = await report(sb, fixtureString(sb, c.dir), env);
      for (const check of c.checks) {
        const value = fixtureString(sb, check.value);
        if (check.op === "equal") expect(out).toBe(value);
        else if (check.op === "contains") expect(out).toContain(value);
        else expect(out).not.toContain(value);
      }
    });
  } else if (raw.kind === "report-error") {
    const c = ErrorSchema.parse(raw);
    test.concurrent(c.name, () => {
      using sb = createSandbox();
      for (const override of c.overrides) {
        const res = Bun.spawnSync([...entryArgv("statusline", "status", PLUGIN), sb.project], {
          env: { ...process.env, HOME: sb.home, TOOLU_HOST_OVERRIDE: override },
          stdout: "pipe",
          stderr: "pipe",
        });
        expect(res.exitCode).not.toBe(0);
        expect(res.stderr.toString()).toContain(`${c.expectedPrefix}${override}`);
      }
    });
  }
}
