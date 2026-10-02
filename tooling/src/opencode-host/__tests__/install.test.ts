import { expect, test } from "bun:test";
import { chmodSync, existsSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { resolveHostBinary } from "../install.ts";
import { contractPaths } from "../results.ts";
import { PinSchema, readJson } from "../schema.ts";

// Real subprocess runs of the live probe CLI against binaries that must be refused (#335).

const ROOT = resolve(import.meta.dir, "../../../..");
const PROBE = join(ROOT, "tooling/src/opencode-host-probe.ts");
const pin = readJson(contractPaths({}).pin, PinSchema);

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
    stderr: `opencode-host-probe: pin mismatch: ${fake} reports 0.0.1, pin is ${pin.cli.version}\n`,
  });
});

function fakeCli(sb: Sandbox, rel: string, version: string): string {
  const bin = sb.write(rel, `#!/bin/sh\necho ${version}\n`);
  chmodSync(bin, 0o755);
  return bin;
}

test.concurrent("an explicit binary reporting the pinned version is used as-is", async () => {
  using sb = createSandbox();
  const bin = fakeCli(sb, "bin/opencode", pin.cli.version);
  expect(await resolveHostBinary(pin, { TOOLU_OPENCODE_HOST_BIN: bin })).toEqual({
    bin,
    version: pin.cli.version,
    installSource: "TOOLU_OPENCODE_HOST_BIN",
  });
});

test.concurrent("a cached pinned CLI is reused without running bun add", async () => {
  using sb = createSandbox();
  const cache = sb.path("cache");
  const bin = fakeCli(
    sb,
    `cache/${pin.cli.version}/cli/node_modules/.bin/opencode`,
    pin.cli.version,
  );
  expect(await resolveHostBinary(pin, { TOOLU_OPENCODE_HOST_CACHE: cache })).toEqual({
    bin,
    version: pin.cli.version,
    installSource: `npm:${pin.cli.package}@${pin.cli.version} (bun add --exact)`,
  });
  expect(existsSync(join(cache, pin.cli.version, "cli/package.json"))).toBe(false);
});

test.concurrent("an unreachable registry fails the install closed", async () => {
  using sb = createSandbox();
  const res = await run([process.execPath, PROBE], {
    cwd: ROOT,
    env: {
      TOOLU_OPENCODE_HOST_CACHE: sb.path("cache"),
      BUN_CONFIG_REGISTRY: "http://127.0.0.1:9",
      BUN_INSTALL_CACHE_DIR: sb.path("bun-cache"),
    },
    timeoutMs: 60_000,
  });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toStartWith(
    `opencode-host-probe: bun add --exact ${pin.cli.package}@${pin.cli.version} failed (exit 1): `,
  );
  expect(existsSync(sb.path(`cache/${pin.cli.version}/cli/node_modules/.bin/opencode`))).toBe(
    false,
  );
});
