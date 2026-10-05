import { expect, test } from "bun:test";
import { chmodSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createSandbox } from "../../harness/sandbox.ts";
import { run } from "../../harness/spawn.ts";

const ROOT = resolve(import.meta.dir, "../../../../..");
const CLI = resolve(ROOT, "tools/toolu-conformance/src/cli/run.ts");

test("the CLI matrix sends protected dispatch through a selected Rust executable", async () => {
  using sb = createSandbox();
  const calls = sb.path("calls.txt");
  const bin = sb.write(
    "bin with spaces/toolu",
    '#!/bin/sh\nprintf "%s\\n" "$*" >> "$PROBE_CALLS"\ncat >/dev/null\nprintf \'{"hookSpecificOutput":{"permissionDecision":"deny"}}\\n\'\n',
  );
  chmodSync(bin, 0o755);
  const result = await run([process.execPath, CLI], {
    env: {
      TOOLU_IMPL: "rust:toolu/pre-tools",
      TOOLU_RUST_BIN_DIR: sb.path("bin with spaces"),
      PROBE_CALLS: calls,
    },
  });
  expect(result.exitCode).toBe(0);
  expect(result.stdout).toContain("protected-files pass");
  expect(result.stdout).toContain("spaces-cwd pass");
  expect(result.stdout).toContain("bootstrap-readiness pass");
  expect(result.stdout).toContain("surface-drift pass");
  expect(readFileSync(calls, "utf8")).toBe("hook pre-tools\nhook pre-tools\n");
});

test("an allow-only selected stub fails the existing deny expectation and names Rust", async () => {
  using sb = createSandbox();
  const bin = sb.write(
    "bin/toolu",
    '#!/bin/sh\ncat >/dev/null\nprintf \'{"hookSpecificOutput":{"permissionDecision":"allow"}}\\n\'\n',
  );
  chmodSync(bin, 0o755);
  const result = await run([process.execPath, CLI], {
    env: { TOOLU_IMPL: "rust:toolu/pre-tools", TOOLU_RUST_BIN_DIR: sb.path("bin") },
  });
  expect(result.exitCode).toBe(1);
  expect(result.stdout).toContain("rust:toolu/pre-tools");
  expect(result.stdout).toContain("expected deny or ask");
});

test("a missing selected binary fails the CLI suite at setup with its expected path", async () => {
  using sb = createSandbox();
  const result = await run([process.execPath, CLI], {
    env: { TOOLU_IMPL: "rust:toolu/pre-tools", TOOLU_RUST_BIN_DIR: sb.path("missing") },
  });
  expect(result.exitCode).toBe(1);
  expect(result.stdout).toContain("rust:toolu/pre-tools");
  expect(result.stdout).toContain(sb.path("missing/toolu"));
});
