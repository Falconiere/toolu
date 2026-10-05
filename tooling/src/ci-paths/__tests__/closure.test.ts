import { expect, test } from "bun:test";
import { relative, resolve } from "node:path";
import { loadRepoCiPaths, matchesAny } from "../config.ts";

// The `opencode` group (#458, AC-8) must cover every first-party module the
// OpenCode acceptance and its live tests import, or an edit to one of them
// would skip the acceptance that exercises it.

const ROOT = resolve(import.meta.dir, "../../../..");
const opencode = loadRepoCiPaths(ROOT).groups.opencode ?? [];

const ENTRY_GLOBS = ["tooling/src/opencode-*.ts", "tools/toolu-opencode/**/*.live.test.ts"];

async function closure(): Promise<string[]> {
  const entrypoints = ENTRY_GLOBS.flatMap((glob) =>
    [...new Bun.Glob(glob).scanSync({ cwd: ROOT })].map((path) => resolve(ROOT, path)),
  );
  expect(entrypoints.length).toBeGreaterThan(10);
  const result = await Bun.build({ entrypoints, target: "bun", metafile: true, throw: false });
  expect(result.logs.filter((log) => log.level === "error").map(String)).toEqual([]);
  const inputs = Object.keys(result.metafile?.inputs ?? {}).map((path) =>
    relative(ROOT, resolve(ROOT, path)),
  );
  return inputs.filter((path) => !path.includes("node_modules/") && !path.startsWith(".."));
}

const uncovered = (inputs: readonly string[], globs: readonly string[]): string[] =>
  inputs.filter((path) => !matchesAny(globs, path));

test("the opencode group covers the acceptance import closure (AC-8)", async () => {
  const inputs = await closure();
  expect(inputs).toContain("tooling/src/opencode-acceptance.ts");
  expect(inputs).toContain("tooling/src/env.ts");
  expect(uncovered(inputs, opencode)).toEqual([]);
  const withoutEnv = opencode.filter((glob) => glob !== "tooling/src/env.ts");
  expect(uncovered(inputs, withoutEnv)).toEqual(["tooling/src/env.ts"]);
}, 120_000);
