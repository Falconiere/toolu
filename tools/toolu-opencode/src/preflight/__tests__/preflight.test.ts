import { expect, test } from "bun:test";
import { mkdtempSync } from "node:fs";
import { join } from "node:path";
import { runPreflight } from "../check.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";

test("preflight reports bash and jq when PATH is normal", async () => {
  const report = await runPreflight();
  expect(report.entries.find((e) => e.tool === "bash")?.present).toBe(true);
  expect(report.entries.find((e) => e.tool === "jq")?.present).toBe(true);
  expect(report.bootstrapAllowed).toBe(true);
});

test("preflight fail closed when jq is not on PATH", async () => {
  const emptyPath = mkdtempSync(join(tmpBase, "toolu-pf-path-"));
  const report = await runPreflight({ env: { PATH: emptyPath } });
  expect(report.bootstrapAllowed).toBe(false);
  expect(report.reasons.some((r) => r.includes("jq"))).toBe(true);
});
