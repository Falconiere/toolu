// Bun workspace skeleton (#208)
import { expect, test } from "bun:test";
import { existsSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { z } from "zod";

const ROOT = resolve(import.meta.dir, "../../..");
const RootPackage = z.object({
  workspaces: z.array(z.string()),
  scripts: z.record(z.string(), z.string()),
});

function readText(rel: string): string {
  return readFileSync(join(ROOT, rel), "utf8");
}

function isDir(rel: string): boolean {
  return existsSync(join(ROOT, rel)) && statSync(join(ROOT, rel)).isDirectory();
}

const rootPackage = RootPackage.parse(JSON.parse(readText("package.json")));

test.concurrent("workspaces list core, opencode, conformance, and cli packages", () => {
  const packages = [
    "packages/toolu-core",
    "tools/toolu-opencode",
    "tools/toolu-conformance",
    "tools/toolu-cli",
  ];
  for (const pkg of packages) {
    expect(rootPackage.workspaces).toContain(pkg);
    expect(existsSync(join(ROOT, pkg, "package.json"))).toBe(true);
    expect(isDir(`${pkg}/src`)).toBe(true);
  }
});

test.concurrent("CI workflow defines a typescript job running test:ts", () => {
  // Same walk the awk did: enter at `  typescript:`, leave at the next two-space key.
  let inJob = false;
  let foundJob = false;
  let foundRun = false;
  for (const line of readText(".github/workflows/tests.yml").split("\n")) {
    if (line.startsWith("  typescript:")) {
      inJob = true;
      foundJob = true;
    } else if (/^ {2}[a-z]/.test(line)) {
      inJob = false;
    } else if (inJob && line.includes("bun run test:ts")) {
      foundRun = true;
    }
  }
  expect({ foundJob, foundRun }).toEqual({ foundJob: true, foundRun: true });
});

test.concurrent("root test script includes test:ts", () => {
  expect(rootPackage.scripts["test:ts"]).toBeTruthy();
  expect(rootPackage.scripts["test"]).toContain("test:ts");
});

test.concurrent("portable-core documents tools/toolu-conformance", () => {
  expect(readText("docs/portable-core.md")).toContain("tools/toolu-conformance");
});

test.concurrent("conventions adoption documents local TS CI commands", () => {
  expect(readText("docs/conventions-adoption.md")).toMatch(/test:ts|typescript/);
});
