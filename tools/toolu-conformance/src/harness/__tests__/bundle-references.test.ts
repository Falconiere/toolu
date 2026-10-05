/**
 * #409: a test reaches a committed hook bundle through `entry-command.ts`, so a
 * `TOOLU_IMPL` selector can put the Rust binary in its place. The only test
 * files that may still spell the bundle path are those whose subject is the
 * bundle file or text naming it, listed below with the reason.
 */
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { run } from "../spawn.ts";

const ROOT = resolve(import.meta.dir, "../../../../..");
// Built from parts so this file does not match its own search.
const NEEDLE = ["hooks", "dist", ""].join("/");
const SPLIT_NEEDLE = ['"hooks"', '"dist"'].join(",\\s*");
const COMMENT = /^\s*(\/\/|\/?\*)/;

/** Path prefix → why the bundle path is the subject rather than a command to run. */
const ALLOWED: Record<string, string> = {
  "tooling/src/__tests__/build-plugins.test.ts": "builds bundles and checks their drift",
  "tooling/src/__tests__/bundle-plugins.test.ts": "checks the bundle file layout",
  "tooling/src/__tests__/pack-closure.test.ts": "checks published tarball contents",
  "tooling/src/__tests__/pack-inventory.test.ts": "checks published tarball file lists",
  "tooling/src/__tests__/check-hooks-json.test.ts": "checks hooks.json launcher text",
  "packages/toolu-core/src/launcher/__tests__/launcher.test.ts":
    "checks the generated launcher text and its fail-closed behavior",
  "packages/toolu-core/src/startup/__tests__/publish.test.ts":
    "builds a fake plugin layout to test stable-path publishing",
  "tools/toolu-opencode/":
    "the OpenCode bootstrap itself spawns bundles; its Rust shim is a later epic issue",
  "plugins/toolu/hooks/src/__tests__/fixtures/pre-tool-modules-c-golden.json":
    "golden command text that names the plan-ledger bundle",
  "plugins/toolu/hooks/src/__tests__/pre-tool-modules-c-plan-ledger.ts":
    "command text that names the plan-ledger bundle",
  "plugins/delivery-flow/skills/__tests__/delivery-flow-contract.test.ts":
    "skill text that names the plan-ledger bundle",
  "plugins/toolu/hooks/src/__tests__/lifecycle-cases.ts": "counts hooks.json launcher entries",
};

/** Test files that build the hooks.json launcher text to compare it, not only to run it. */
const LAUNCHER_ALLOWED: Record<string, string> = {
  "packages/toolu-core/src/launcher/__tests__/launcher.test.ts":
    "the launcher itself is the subject",
  "tooling/src/__tests__/launcher-host-schemas.test.ts":
    "the launcher's own outputs against host schemas",
  "plugins/toolu/hooks/src/__tests__/pre-tools-wiring.test.ts":
    "compares hooks.json commands with the launcher text",
  "plugins/toolu/hooks/src/__tests__/post-tools-wiring.test.ts":
    "compares hooks.json commands with the launcher text",
};

function allowed(file: string, list: Record<string, string> = ALLOWED): boolean {
  return Object.keys(list).some((prefix) => file.startsWith(prefix));
}

async function references(pattern = `${NEEDLE}|${SPLIT_NEEDLE}`): Promise<Map<string, string[]>> {
  const result = await run(
    ["git", "grep", "-n", "-E", pattern, "--", "**/__tests__/**", "*.test.ts"],
    { cwd: ROOT },
  );
  expect(result.stderr).toBe("");
  const found = new Map<string, string[]>();
  for (const line of result.stdout.split("\n")) {
    const match = /^([^:]+):(\d+):(.*)$/.exec(line);
    if (match === null) continue;
    const [, file = "", lineNo = "", text = ""] = match;
    if (COMMENT.test(text)) continue;
    found.set(file, [...(found.get(file) ?? []), `${lineNo}: ${text.trim()}`]);
  }
  return found;
}

test("test files reach committed bundles only through the entry-command resolver", async () => {
  const found = await references();
  const offenders = [...found].filter(([file]) => !allowed(file));
  expect(offenders).toEqual([]);
});

test("every allowlist entry still names a bundle path", async () => {
  const files = [...(await references()).keys()];
  const stale = Object.keys(ALLOWED).filter(
    (prefix) => !files.some((file) => file.startsWith(prefix)),
  );
  expect(stale).toEqual([]);
});

test("tests run a hook's launcher only through launchedArgv", async () => {
  const found = await references(String.raw`\blauncherCommand\(`);
  const offenders = [...found].filter(([file]) => !allowed(file, LAUNCHER_ALLOWED));
  expect(offenders).toEqual([]);
  const stale = Object.keys(LAUNCHER_ALLOWED).filter((prefix) => !found.has(prefix));
  expect(stale).toEqual([]);
});
