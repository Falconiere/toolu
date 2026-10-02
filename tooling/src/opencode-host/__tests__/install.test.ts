import { expect, test } from "bun:test";
import { chmodSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";

// Real subprocess runs of the live probe CLI against binaries that must be refused (#335).

const ROOT = resolve(import.meta.dir, "../../../..");
const PROBE = join(ROOT, "tooling/src/opencode-host-probe.ts");

test.concurrent("a missing TOOLU_OPENCODE_HOST_BIN fails closed without installing anything", async () => {
  using sb = createSandbox();
  const cache = sb.write("cache/.keep", "");
  const res = await run([process.execPath, PROBE], {
    cwd: ROOT,
    env: {
      TOOLU_OPENCODE_HOST_BIN: "/nonexistent/opencode",
      TOOLU_OPENCODE_HOST_CACHE: join(cache, ".."),
    },
    timeoutMs: 30_000,
  });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toStartWith("opencode-host-probe: cannot run /nonexistent/opencode --version");
  expect(readdirSync(join(cache, ".."))).toEqual([".keep"]);
});

test.concurrent("a binary reporting another version is a pin mismatch", async () => {
  using sb = createSandbox();
  const fake = sb.write("bin/opencode", "#!/bin/sh\necho 0.0.1\n");
  chmodSync(fake, 0o755);
  const res = await run([process.execPath, PROBE], {
    cwd: ROOT,
    env: { TOOLU_OPENCODE_HOST_BIN: fake, TOOLU_OPENCODE_HOST_CACHE: sb.path("cache") },
    timeoutMs: 30_000,
  });
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({
    exitCode: 1,
    stderr: `opencode-host-probe: pin mismatch: ${fake} reports 0.0.1, pin is 1.18.34\n`,
  });
});
