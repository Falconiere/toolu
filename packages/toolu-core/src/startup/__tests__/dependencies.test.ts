/**
 * Codex dependency checks against a real `codex` executable on PATH printing
 * the `plugin list --json` shapes the bash `check-toolu.sh` jq filter judged
 * (#269). Parity with the remaining bash copy is asserted in pr-babysit's suite.
 */
import { afterAll, expect, test } from "bun:test";
import { chmodSync, mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  codexDependencyNotice,
  codexMissingPlugins,
  requiresCoreWarning,
  requiresPluginsWarning,
} from "../dependencies.ts";

const temps: string[] = [];
afterAll(() => {
  for (const dir of temps) rmSync(dir, { recursive: true, force: true });
});

function temp(): string {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), "startup-deps-")));
  temps.push(dir);
  return dir;
}

/** Codex-host env whose `codex` prints `stdout` and exits `code`. */
function codexEnv(stdout: string, code = 0): Record<string, string> {
  const root = temp();
  const bin = join(root, "bin");
  mkdirSync(bin);
  writeFileSync(join(bin, "codex"), `#!/bin/sh\ncat <<'JSON'\n${stdout}\nJSON\nexit ${code}\n`);
  chmodSync(join(bin, "codex"), 0o755);
  return { PATH: `${bin}:/usr/bin:/bin`, HOME: root, PLUGIN_ROOT: join(root, "plugin") };
}

const CORE = ["toolu@toolu"];
const entry = (flags: object): string =>
  JSON.stringify({ installed: [{ pluginId: "toolu@toolu", ...flags }] });

test.concurrent("installed and enabled (or flags absent) is not missing", () => {
  expect(codexMissingPlugins(CORE, codexEnv(entry({ installed: true, enabled: true })))).toEqual(
    [],
  );
  expect(codexMissingPlugins(CORE, codexEnv(entry({})))).toEqual([]);
});

test.concurrent("a disabled or not-installed record is missing", () => {
  expect(codexMissingPlugins(CORE, codexEnv(entry({ installed: false })))).toEqual(CORE);
  expect(codexMissingPlugins(CORE, codexEnv(entry({ enabled: false })))).toEqual(CORE);
  expect(codexMissingPlugins(CORE, codexEnv(entry({ enabled: "true" })))).toEqual(CORE);
});

test.concurrent("an empty or absent installed list is missing", () => {
  expect(codexMissingPlugins(CORE, codexEnv('{"installed":[]}'))).toEqual(CORE);
  expect(codexMissingPlugins(CORE, codexEnv("{}"))).toEqual(CORE);
  expect(codexMissingPlugins(CORE, codexEnv("[1,2]"))).toEqual(CORE);
});

test.concurrent("unparseable output counts as missing, as jq -e failing did", () => {
  expect(codexMissingPlugins(CORE, codexEnv("not json"))).toEqual(CORE);
  expect(codexMissingPlugins(CORE, codexEnv(""))).toEqual(CORE);
});

test.concurrent("null entries are skipped; an unindexable entry raises unless a match came first", () => {
  const match = { pluginId: "toolu@toolu" };
  expect(codexMissingPlugins(CORE, codexEnv(JSON.stringify({ installed: [null, match] })))).toEqual(
    [],
  );
  expect(codexMissingPlugins(CORE, codexEnv(JSON.stringify({ installed: ["x", match] })))).toEqual(
    CORE,
  );
  expect(codexMissingPlugins(CORE, codexEnv(JSON.stringify({ installed: [match, "x"] })))).toEqual(
    [],
  );
  expect(codexMissingPlugins(CORE, codexEnv(JSON.stringify({ installed: { a: match } })))).toEqual(
    [],
  );
});

test.concurrent("no check off Codex, without PLUGIN_ROOT, without the CLI, or when it fails", () => {
  const env = codexEnv('{"installed":[]}');
  expect(codexMissingPlugins(CORE, { ...env, TOOLU_HOST_OVERRIDE: "claude" })).toBeUndefined();
  expect(codexMissingPlugins(CORE, { ...env, PLUGIN_ROOT: "" })).toBeUndefined();
  expect(
    codexMissingPlugins(CORE, { ...env, PLUGIN_ROOT: "", TOOLU_HOST_OVERRIDE: "codex" }),
  ).toBeUndefined();
  expect(codexMissingPlugins(CORE, { ...env, PATH: temp() })).toBeUndefined();
  expect(codexMissingPlugins(CORE, codexEnv('{"installed":[]}', 1))).toBeUndefined();
});

test.concurrent("missing ids keep the required order", () => {
  const listed = JSON.stringify({ installed: [{ pluginId: "b@toolu" }] });
  expect(codexMissingPlugins(["c@toolu", "b@toolu", "a@toolu"], codexEnv(listed))).toEqual([
    "c@toolu",
    "a@toolu",
  ]);
});

test.concurrent("warnings name the exact Codex install commands", () => {
  expect(requiresCoreWarning()).toBe(
    "WARN: this plugin requires the toolu core. Install it first with: codex plugin add toolu@toolu",
  );
  expect(requiresPluginsWarning(["toolu@toolu", "brainstorm@toolu"])).toBe(
    "WARN: this plugin requires toolu@toolu (install with: codex plugin add toolu@toolu) brainstorm@toolu (install with: codex plugin add brainstorm@toolu)",
  );
});

test.concurrent("the notice is jq-pretty SessionStart context, or nothing", () => {
  const missing = codexEnv('{"installed":[]}');
  expect(codexDependencyNotice(CORE, "core", missing)).toBe(
    `${JSON.stringify(
      {
        hookSpecificOutput: {
          hookEventName: "SessionStart",
          additionalContext: requiresCoreWarning(),
        },
      },
      null,
      2,
    )}\n`,
  );
  expect(codexDependencyNotice(CORE, "core", codexEnv(entry({})))).toBeUndefined();
  expect(codexDependencyNotice(["x@toolu"], "each", missing)).toContain(
    "(install with: codex plugin add x@toolu)",
  );
});
