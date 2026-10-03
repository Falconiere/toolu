// Conventions guardrails (#213) -- real tools against fixture trees.
import { expect, test } from "bun:test";
import { copyFileSync, mkdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";

const ROOT = resolve(import.meta.dir, "../../..");
const FIX = join(ROOT, "tooling/fixtures/conventions");
const GR = join(ROOT, "tooling/src/guardrails/run.ts");
const RootPackage = z.object({
  scripts: z.record(z.string(), z.string()),
  dependencies: z.record(z.string(), z.string()),
});

function readText(rel: string): string {
  return readFileSync(join(ROOT, rel), "utf8");
}

test.concurrent("adoption docs record pin, thresholds, and Lefthook note", () => {
  expect(readText("tooling/conventions/PROVENANCE.md")).toContain("3562c63");
  const adoption = readText("docs/conventions-adoption.md");
  expect(adoption).toContain("300");
  expect(adoption).toContain("60");
  expect(adoption).toContain("Lefthook");
});

test.concurrent("package.json wires test:conventions and zod without yup", () => {
  const pkg = RootPackage.parse(JSON.parse(readText("package.json")));
  expect(pkg.scripts["test:conventions"]).toBeTruthy();
  expect(pkg.scripts["test:ts"]).toContain("test:conventions");
  expect(pkg.scripts["test"]).toContain("test:ts");
  expect(pkg.dependencies["zod"]).toBeTruthy();
  expect(Object.keys(pkg.dependencies)).not.toContain("yup");
});

test.concurrent("root workspace passes banned-deps", async () => {
  const res = await run([process.execPath, GR, "--only", "banned-deps"], { cwd: ROOT });
  expect({ exitCode: res.exitCode, output: res.stdout + res.stderr }).toEqual({
    exitCode: 0,
    output: res.stdout + res.stderr,
  });
});

test.concurrent("violating fixture with valibot fails banned-deps in temp tree", async () => {
  using sb = createSandbox();
  mkdirSync(sb.path("src/utilities"), { recursive: true });
  copyFileSync(join(FIX, "violating/package.json"), sb.path("package.json"));
  copyFileSync(join(FIX, "violating/guardrails.config.json"), sb.path("guardrails.config.json"));
  copyFileSync(join(FIX, "clean/src/utilities/parse-id.ts"), sb.path("src/utilities/parse-id.ts"));
  const res = await run([process.execPath, GR, "--only", "banned-deps"], { cwd: sb.project });
  expect(res.exitCode).not.toBe(0);
});

test.concurrent("oxlint fails on explicit any and type assertion fixture", async () => {
  // Prefer bunx so CI (after bun install) resolves the locked oxlint binary.
  const res = await run(
    [
      "bunx",
      "oxlint",
      "-c",
      join(FIX, "oxlint.fixture.json"),
      "--deny-warnings",
      join(FIX, "violating/src/utilities/bad.ts"),
    ],
    { cwd: ROOT },
  );
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toMatch(/any|assertion|explicit|typescript/i);
});

test.concurrent("production tooling sources have no type assertions", () => {
  // The pattern is assembled from pieces so this file does not trip its own scan.
  const keyword = ["a", "s"].join("");
  const assertion = new RegExp(` ${keyword} [A-Za-z{]`);
  const constAssertion = ` ${keyword} const`;
  const offenders: string[] = [];
  const glob = new Bun.Glob("**/*.ts");
  for (const rel of glob.scanSync({ cwd: join(ROOT, "tooling/src") })) {
    const lines = readFileSync(join(ROOT, "tooling/src", rel), "utf8").split("\n");
    lines.forEach((line, index) => {
      if (assertion.test(line) && !line.includes(constAssertion)) {
        offenders.push(`${rel}:${String(index + 1)}:${line}`);
      }
    });
  }
  expect(offenders).toEqual([]);
});

test.concurrent("lint-suppressions --file fails on unused-vars disable", async () => {
  using sb = createSandbox();
  copyFileSync(join(FIX, "clean/guardrails.config.json"), sb.path("guardrails.config.json"));
  sb.write("package.json", '{ "name": "suppress-fixture", "private": true }\n');
  sb.write(
    "src/utilities/suppressed.ts",
    [
      "/** Intentional unused-vars suppression for fixture. */",
      "// oxlint-disable-next-line eslint/no-unused-vars",
      "const suppressed = 1;",
      "export function ok(): number {",
      "  return 1;",
      "}",
      "",
    ].join("\n"),
  );
  const res = await run(
    [process.execPath, GR, "--only", "lint-suppressions", "--file", "src/utilities/suppressed.ts"],
    { cwd: sb.project },
  );
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain("lint-suppressions");
});

test.concurrent("test:conventions runs the reach and legacy-exemption checks, and the adoption doc names them", () => {
  const pkg = RootPackage.parse(JSON.parse(readText("package.json")));
  const adoption = readText("docs/conventions-adoption.md");
  for (const script of ["check:gate-reach", "check:legacy-exemptions"]) {
    expect(pkg.scripts[script]).toBeTruthy();
    expect(pkg.scripts["test:conventions"]).toContain(`bun run ${script}`);
    expect(adoption).toContain(script);
  }
});
