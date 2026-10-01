import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { resolveBunExecutable, runPreflight } from "../check.ts";

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
  const report = runPreflight({ env: { PATH: emptyPath, HOME: emptyPath, TOOLU_BUN: "" } });
  expect(report.bootstrapAllowed).toBe(false);
  expect(report.reasons).toEqual(["missing required tool: git", "missing required tool: bun"]);
});

test("Bun resolution prefers TOOLU_BUN, then PATH, then host HOME", () => {
  const root = mkdtempSync(join(tmpBase, "toolu-pf-resolve-"));
  const pathBin = join(root, "path");
  const home = join(root, "home");
  mkdirSync(pathBin);
  mkdirSync(join(home, ".bun", "bin"), { recursive: true });
  const explicitBun = join(root, "explicit-bun");
  const pathBun = join(pathBin, "bun");
  const homeBun = join(home, ".bun", "bin", "bun");
  for (const path of [explicitBun, pathBun, homeBun]) symlinkSync(process.execPath, path);
  const env = { TOOLU_BUN: explicitBun, PATH: pathBin, HOME: home };
  expect(resolveBunExecutable(env)).toBe(explicitBun);
  expect(resolveBunExecutable({ ...env, TOOLU_BUN: "" })).toBe(pathBun);
  expect(resolveBunExecutable({ ...env, TOOLU_BUN: "", PATH: root })).toBe(homeBun);
});

test("a non-executable TOOLU_BUN falls through, and no executable Bun fails closed", () => {
  const root = mkdtempSync(join(tmpBase, "toolu-pf-invalid-"));
  const badBun = join(root, "bad-bun");
  writeFileSync(badBun, "not executable\n");
  chmodSync(badBun, 0o644);
  const bin = join(root, "bin");
  mkdirSync(bin);
  symlinkSync(process.execPath, join(bin, "bun"));
  expect(resolveBunExecutable({ TOOLU_BUN: badBun, PATH: bin, HOME: root })).toBe(join(bin, "bun"));
  expect(resolveBunExecutable({ TOOLU_BUN: badBun, PATH: root, HOME: root })).toBeNull();
});
