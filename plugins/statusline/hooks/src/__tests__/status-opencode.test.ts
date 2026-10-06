/** OpenCode status reports over shared startup record states. */
import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { materializeCaseValue, readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { put, report, repo } from "./harness.ts";

const CaseSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("opencode-status"),
  records: z.array(z.json()),
  gate: z.string().optional(),
  host: z.enum(["opencode", "codex"]),
  checks: z
    .array(
      z.strictObject({ op: z.enum(["equal", "starts", "contains", "absent"]), value: z.json() }),
    )
    .min(1),
});
const cases = readCaseFile(
  resolve(import.meta.dir, "../../../../../fixtures/statusline/cases.json"),
)
  .filter((raw) => raw.kind === "opencode-status")
  .map((raw) => CaseSchema.parse(raw));

function dataRoot(sb: Sandbox): string {
  return join(sb.project, ".opencode/toolu/state");
}
function recordAt(sb: Sandbox, body: string): void {
  put(join(dataRoot(sb), "toolu/opencode-status.json"), body);
}

for (const c of cases) {
  test.concurrent(c.name, async () => {
    for (const record of c.records.length === 0 ? [undefined] : c.records) {
      using sb = createSandbox();
      repo(sb.project);
      if (record !== undefined) {
        const value = materializeCaseValue(sb, record);
        recordAt(sb, typeof value === "string" ? value : JSON.stringify(value));
      }
      if (c.gate !== undefined) sb.write(".opencode/tmp/quality-gate-status.json", c.gate);
      const out = await report(sb, sb.project, {
        TOOLU_HOST_OVERRIDE: c.host,
        TOOLU_CONFIG_DIR: dataRoot(sb),
      });
      for (const check of c.checks) {
        const value = z.string().parse(materializeCaseValue(sb, check.value));
        if (check.op === "equal") expect(out).toBe(value);
        else if (check.op === "starts") expect(out).toStartWith(value);
        else if (check.op === "contains") expect(out).toContain(value);
        else expect(out).not.toContain(value);
      }
    }
  });
}
