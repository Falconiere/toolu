import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";

// Drives the real PreToolUse dispatcher through tooling/src/probe-ask.ts.

const ROOT = resolve(import.meta.dir, "../../..");

test.concurrent("a hook ask survives a blanket Bash(*) allowlist", async () => {
  const res = await run([process.execPath, resolve(ROOT, "tooling/src/probe-ask.ts")], {
    cwd: ROOT,
    timeoutMs: 60_000,
  });
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("probe-ask: ask emitted with Bash(*) allowlisted");
});
