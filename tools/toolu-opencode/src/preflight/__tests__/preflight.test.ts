import { expect, test } from "bun:test";
import { mkdtempSync, symlinkSync } from "node:fs";
import { join } from "node:path";
import { runPreflight } from "../check.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";

test("preflight reports git and Bun when PATH is normal", () => {
  const report = runPreflight();
  expect(report.entries.find((e) => e.tool === "git")?.present).toBe(true);
  expect(report.entries.find((e) => e.tool === "bun")?.present).toBe(true);
  expect(report.bootstrapAllowed).toBe(true);
});

test("preflight permits a PATH with git and Bun but no bash or jq", () => {
  const bin = mkdtempSync(join(tmpBase, "toolu-pf-path-"));
  symlinkSync(Bun.which("git") ?? "/usr/bin/git", join(bin, "git"));
  symlinkSync(process.execPath, join(bin, "bun"));
  const report = runPreflight({ env: { PATH: bin } });
  expect(report.bootstrapAllowed).toBe(true);
  expect(report.entries.map((entry) => entry.tool)).toEqual(["git", "bun", "opencode"]);
});

test("preflight fails closed when git and Bun are absent", () => {
  const emptyPath = mkdtempSync(join(tmpBase, "toolu-pf-path-"));
  const report = runPreflight({ env: { PATH: emptyPath } });
  expect(report.bootstrapAllowed).toBe(false);
  expect(report.reasons).toEqual(["missing required tool: git", "missing required tool: bun"]);
});
