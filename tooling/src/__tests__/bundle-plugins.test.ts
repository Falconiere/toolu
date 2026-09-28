import { expect, test } from "bun:test";
import { existsSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import type { RunResult } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";

// The published @toolu/opencode carries the bash plugins/ tree, because npm
// cannot reach outside a package directory and the bridge enforces those gates.
//
// Lives in tooling/, not beside the script it tests: Bun symlinks workspace
// packages into each other's node_modules, so a test file inside
// tools/toolu-opencode/ is discovered twice -- once for real and once through
// tools/toolu-conformance/node_modules/@toolu/opencode/, where the relative
// walk up to the repo root lands inside node_modules and the script is absent.

const ROOT = resolve(import.meta.dir, "../../..");
const SCRIPT = join(ROOT, "tools/toolu-opencode/scripts/bundle-plugins.sh");
const Catalog = z.object({ plugins: z.array(z.unknown()) });

/**
 * Stage into a private directory per test: pack-inventory triggers the same
 * script through prepack, and tests here run concurrently.
 */
function stage(sb: Sandbox): { dest: string; bundle: () => Promise<RunResult> } {
  const dest = join(sb.root, "staged");
  return {
    dest,
    bundle: () => run(["bash", SCRIPT], { cwd: ROOT, env: { BUNDLE_PLUGINS_DEST: dest } }),
  };
}

/** Every directory under `base`, in path form with "/" separators. */
function directories(base: string): string[] {
  return readdirSync(base, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => join(entry.parentPath, entry.name));
}

function isTests(dir: string): boolean {
  return dir.endsWith("/__tests__");
}

function isFile(path: string): boolean {
  return statSync(path, { throwIfNoEntry: false })?.isFile() === true;
}

test.concurrent("bundles every marketplace plugin", async () => {
  using sb = createSandbox();
  const { dest, bundle } = stage(sb);
  const res = await bundle();
  expect(res.exitCode).toBe(0);
  const catalog = Catalog.parse(
    JSON.parse(await Bun.file(join(ROOT, ".claude-plugin/marketplace.json")).text()),
  );
  const staged = readdirSync(dest, { recursive: true, encoding: "utf8" }).filter((rel) =>
    rel.endsWith(".claude-plugin/plugin.json"),
  );
  expect(staged.length).toBe(catalog.plugins.length);
});

test.concurrent("the staged tree carries the hook engine the bridge actually runs", async () => {
  using sb = createSandbox();
  const { dest, bundle } = stage(sb);
  expect((await bundle()).exitCode).toBe(0);
  expect(isFile(join(dest, "toolu/hooks/hooks.json"))).toBe(true);
  expect(isFile(join(dest, "toolu/hooks/pre-tools/mod.sh"))).toBe(true);
  expect(statSync(join(dest, "toolu/settings")).isDirectory()).toBe(true);
});

test.concurrent("colocated tests are excluded from the published copy", async () => {
  using sb = createSandbox();
  const { dest, bundle } = stage(sb);
  expect((await bundle()).exitCode).toBe(0);
  expect(directories(dest).filter(isTests)).toEqual([]);
  // The source tree really does have them, so the exclusion is doing work.
  expect(directories(join(ROOT, "plugins")).filter(isTests).length).toBeGreaterThan(0);
});

test.concurrent("hook sources are excluded while their built bundles ship", async () => {
  using sb = createSandbox();
  const { dest, bundle } = stage(sb);
  expect((await bundle()).exitCode).toBe(0);
  expect(directories(dest).filter((dir) => dir.endsWith("/hooks/src"))).toEqual([]);
  expect(isFile(join(dest, "toolu/hooks/dist/sample.js"))).toBe(true);
  // The source tree really does have them, so the exclusion is doing work.
  expect(isFile(join(ROOT, "plugins/toolu/hooks/src/sample.ts"))).toBe(true);
});

test.concurrent("a re-run replaces the copy rather than accumulating stale files", async () => {
  using sb = createSandbox();
  const { dest, bundle } = stage(sb);
  expect((await bundle()).exitCode).toBe(0);
  writeFileSync(join(dest, "stale-marker"), "");
  expect((await bundle()).exitCode).toBe(0);
  expect(existsSync(join(dest, "stale-marker"))).toBe(false);
});

test.concurrent("the package's own staged copy is gitignored so it never lands in a commit", async () => {
  // Assert on a path INSIDE the directory. The .gitignore pattern is `plugins/`,
  // and git only matches a trailing-slash pattern against a path it can see is a
  // directory -- so checking the bare directory passes locally, where a previous
  // run created it, and fails on a clean CI checkout where it does not exist.
  const res = await run(
    [
      "git",
      "-C",
      ROOT,
      "check-ignore",
      "-q",
      "tools/toolu-opencode/plugins/toolu/hooks/hooks.json",
    ],
    { cwd: ROOT },
  );
  expect(res.exitCode).toBe(0);
});
