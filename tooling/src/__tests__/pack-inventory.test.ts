import { afterEach, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { checkOne, expectations, HOOK_SOURCES, packedFiles } from "../pack-inventory.ts";
import type { Expectation } from "../pack-inventory.ts";

const ROOT = resolve(import.meta.dir, "../../..");
const temps: string[] = [];

/** A real package on disk that publishes its plugins/ tree. */
function fixturePackage(files: readonly string[]): string {
  const dir = mkdtempSync(join(tmpdir(), "pack-inventory-"));
  temps.push(dir);
  const manifest = { name: "pack-fixture", version: "0.0.0", files: ["plugins"] };
  writeFileSync(join(dir, "package.json"), `${JSON.stringify(manifest)}\n`);
  for (const file of files) {
    mkdirSync(dirname(join(dir, file)), { recursive: true });
    writeFileSync(join(dir, file), "export {};\n");
  }
  return dir;
}

function expectation(dir: string, required: readonly string[]): Expectation {
  return {
    dir,
    name: "pack-fixture",
    required,
    forbidden: [],
    forbiddenPatterns: [HOOK_SOURCES],
    exact: false,
  };
}

afterEach(() => {
  for (const dir of temps.splice(0)) rmSync(dir, { recursive: true, force: true });
});

test("a tarball carrying hook sources and missing a bundle is rejected on both counts", () => {
  const dir = fixturePackage(["plugins/x/hooks/src/a.ts", "plugins/x/hooks/dist/a.js"]);
  const files = packedFiles(dir);
  expect(files).toContain("plugins/x/hooks/src/a.ts");

  const required = ["package.json", "plugins/x/hooks/dist/a.js", "plugins/x/hooks/dist/b.js"];
  expect(checkOne(expectation(dir, required), files)).toEqual([
    "pack-fixture tarball is missing plugins/x/hooks/dist/b.js",
    "pack-fixture tarball must not contain plugins/x/hooks/src/a.ts",
  ]);
});

test("a tarball carrying only built bundles passes", () => {
  const dir = fixturePackage(["plugins/x/hooks/dist/a.js", "plugins/x/scripts/src/tool.ts"]);
  const required = ["package.json", "plugins/x/hooks/dist/a.js"];
  expect(checkOne(expectation(dir, required), packedFiles(dir))).toEqual([]);
});

test("the opencode tarball must carry every committed bundle and no hook source", () => {
  const opencode = expectations(ROOT).find((candidate) => candidate.name === "@toolu/opencode");
  expect(opencode?.required).toContain("plugins/toolu/hooks/dist/sample.js");
  expect(opencode?.forbiddenPatterns).toContain(HOOK_SOURCES);
});

test("the CLI tarball stays exact and also forbids hook sources", () => {
  const cli = expectations(ROOT).find((candidate) => candidate.name === "@toolu/plugins");
  expect(cli?.exact).toBe(true);
  expect(cli?.forbidden).toContain("plugins/");
  expect(cli?.forbiddenPatterns).toContain(HOOK_SOURCES);
});
