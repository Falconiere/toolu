import { expect, test } from "bun:test";
import { chmodSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";

// Real-data checks for tooling/src/opencode-capability-probe.ts (#205).

const ROOT = resolve(import.meta.dir, "../../..");
const PROBE = join(ROOT, "tooling/src/opencode-capability-probe.ts");
const PROBE_ARGV = [process.execPath, "run", PROBE];
const PROBE_TIMEOUT_MS = 10_000;

test.concurrent("capability probe fixture mode passes against committed doc+fixture", async () => {
  const res = await run(PROBE_ARGV, { cwd: ROOT, env: { PORTABLE_CORE_PROBE_MODE: "fixture" } });
  expect(res.exitCode).toBe(0);
  expect(res.stdout + res.stderr).toContain("fixture ok");
});

test.concurrent("capability probe fails closed for nonexistent OPENCODE_BIN within 10s", async () => {
  const res = await run(PROBE_ARGV, {
    cwd: ROOT,
    env: { PORTABLE_CORE_PROBE_MODE: undefined, OPENCODE_BIN: "/nonexistent/opencode" },
    timeoutMs: PROBE_TIMEOUT_MS,
  });
  expect(res.timedOut).toBe(false);
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).not.toMatch(/enforced/i);
});

test.concurrent("capability probe fails on pin mismatch when fake CLI prints wrong version", async () => {
  using sb = createSandbox();
  const fake = sb.write("fake-opencode", "#!/bin/sh\necho opencode v0.0.1\n");
  chmodSync(fake, 0o755);
  const res = await run(PROBE_ARGV, {
    cwd: ROOT,
    env: { PORTABLE_CORE_PROBE_MODE: undefined, OPENCODE_BIN: fake },
  });
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).not.toMatch(/enforced/i);
});
