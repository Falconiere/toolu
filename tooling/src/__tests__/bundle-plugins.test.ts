import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import type { RunResult } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { stagePlugins } from "../../../tools/toolu-opencode/scripts/bundle-plugins.ts";
import { pluginStartupEntries } from "../../../tools/toolu-opencode/src/bootstrap/entrypoint.ts";

// The published @toolu/opencode carries manifests, runtime data and bundles.
//
// Lives in tooling/, not beside the script it tests: Bun symlinks workspace
// packages into each other's node_modules, so a test file inside
// tools/toolu-opencode/ is discovered twice -- once for real and once through
// tools/toolu-conformance/node_modules/@toolu/opencode/, where the relative
// walk up to the repo root lands inside node_modules and the script is absent.

const ROOT = resolve(import.meta.dir, "../../..");
const SCRIPT = join(ROOT, "tools/toolu-opencode/scripts/bundle-plugins.ts");
const Catalog = z.object({ plugins: z.array(z.unknown()) });

/**
 * Stage into a private directory per test: pack-inventory triggers the same
 * script through prepack, and tests here run concurrently.
 */
function stage(sb: Sandbox): { dest: string; bundle: () => Promise<RunResult> } {
  const dest = join(sb.root, "staged");
  return {
    dest,
    bundle: () => run(["bun", SCRIPT], { cwd: ROOT, env: { BUNDLE_PLUGINS_DEST: dest } }),
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

test.concurrent("the staged tree carries native bundles and required settings", async () => {
  using sb = createSandbox();
  const { dest, bundle } = stage(sb);
  expect((await bundle()).exitCode).toBe(0);
  expect(isFile(join(dest, "toolu/hooks/hooks.json"))).toBe(true);
  expect(isFile(join(dest, "toolu/hooks/dist/pre-tools.js"))).toBe(true);
  expect(statSync(join(dest, "toolu/settings")).isDirectory()).toBe(true);
});

test.concurrent("the debug skill's helpers ship and run from the staged copy", async () => {
  using sb = createSandbox();
  const { dest, bundle } = stage(sb);
  expect((await bundle()).exitCode).toBe(0);
  expect(readdirSync(join(dest, "toolu/scripts")).sort()).toEqual([
    "debug-io.ts",
    "debug-log.ts",
    "debug-stack.ts",
    "debug-testfail.ts",
  ]);
  const transcript = join(ROOT, "plugins/toolu/scripts/__tests__/fixtures/debug/bun-testfail.txt");
  const res = await run(
    ["bun", join(dest, "toolu/scripts/debug-testfail.ts"), "--file", transcript],
    {
      cwd: sb.root,
    },
  );
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("  - add sums two numbers\n");
  expect(res.stdout).toContain("math.test.ts:4:21");
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

test.concurrent("no bash, bats or shell files enter the staged catalog", async () => {
  using sb = createSandbox();
  const { dest, bundle } = stage(sb);
  expect((await bundle()).exitCode).toBe(0);
  const staged = readdirSync(dest, { recursive: true, encoding: "utf8" });
  expect(staged.filter((file) => /\.(sh|bash|bats)$/.test(file))).toEqual([]);
});

test.concurrent("a legacy-only plugin stages its hooks.json routing without its shell hook", () => {
  using sb = createSandbox();
  const source = join(sb.root, "source");
  const plugin = join(source, "legacy-only");
  const dest = join(sb.root, "staged");
  mkdirSync(join(plugin, ".claude-plugin"), { recursive: true });
  mkdirSync(join(plugin, "hooks"), { recursive: true });
  writeFileSync(join(plugin, ".claude-plugin", "plugin.json"), "{}\n");
  writeFileSync(join(plugin, "hooks", "register.sh"), "#!/usr/bin/env bash\nexit 0\n");
  const command = 'bash "${CLAUDE_PLUGIN_ROOT}/hooks/register.sh"';
  const routing = { hooks: { SessionStart: [{ hooks: [{ type: "command", command }] }] } };
  writeFileSync(join(plugin, "hooks", "hooks.json"), JSON.stringify(routing));
  expect(stagePlugins(source, dest)).toBe(1);
  expect(existsSync(join(dest, "legacy-only/hooks/register.sh"))).toBe(false);
  // The staged routing still names the shell hook, so the OpenCode startup refuses the plugin.
  expect(pluginStartupEntries(join(dest, "legacy-only"))).toEqual({
    ok: false,
    reason:
      `unsupported SessionStart command ${JSON.stringify(command)}; ` +
      "regenerate it with `bun run tooling/src/check-hooks-json.ts --print legacy-only SessionStart <entry>`",
  });
});

test.concurrent("staging refuses a destination inside the source catalog", () => {
  using sb = createSandbox();
  const source = join(sb.root, "source");
  const dest = join(source, "occupied");
  mkdirSync(dest, { recursive: true });
  writeFileSync(join(dest, "keep.txt"), "keep\n");
  expect(() => stagePlugins(source, dest)).toThrow("unsafe bundle destination");
  expect(readFileSync(join(dest, "keep.txt"), "utf8")).toBe("keep\n");
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
