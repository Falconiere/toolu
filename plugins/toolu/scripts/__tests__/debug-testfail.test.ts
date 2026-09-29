/** debug-testfail.ts against REAL captured bun + cargo failure transcripts
 * (see fixtures/debug/PROVENANCE.md). No synthetic test output. */

import { expect, test } from "bun:test";
import { join } from "node:path";
import { run, type RunOptions } from "@toolu/conformance/harness/spawn";

const SCRIPT = join(import.meta.dir, "..", "debug-testfail.ts");
const FX = join(import.meta.dir, "fixtures", "debug");
const BUN = join(FX, "bun-testfail.txt");
const CARGO = join(FX, "cargo-testfail.txt");

const debugTestfail = (args: string[], opts: RunOptions = {}) =>
  run(["bun", SCRIPT, ...args], opts);

test.concurrent("bun: surfaces both failed test names", async () => {
  const res = await debugTestfail(["--file", BUN]);
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("  - add sums two numbers\n");
  expect(res.stdout).toContain("  - add handles zero\n");
});

test.concurrent("bun: surfaces the assertion error and a file:line location", async () => {
  const res = await debugTestfail(["--file", BUN]);
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("expect(received).toBe(expected)");
  expect(res.stdout).toContain("math.test.ts:4:21");
});

test.concurrent("cargo: surfaces the failed test name with the test prefix stripped", async () => {
  const res = await debugTestfail(["--file", CARGO]);
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("tests::adds");
  expect(res.stdout).not.toContain("test tests::adds");
});

test.concurrent("cargo: surfaces the panic location src/lib.rs:6:17", async () => {
  const res = await debugTestfail(["--file", CARGO]);
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("src/lib.rs:6:17");
});

test.concurrent("json mode emits recognized:true with the location", async () => {
  const res = await debugTestfail(["--json", "--file", CARGO]);
  expect(res.exitCode).toBe(0);
  const out = JSON.parse(res.stdout) as { recognized: boolean; locations: string[] };
  expect(out.recognized).toBe(true);
  expect(out.locations).toContain("src/lib.rs:6:17");
});

test.concurrent("language-agnostic: same script parses both TS and Rust without flags", async () => {
  expect((await debugTestfail(["--file", BUN])).stdout).toContain("FAILED TESTS");
  expect((await debugTestfail(["--file", CARGO])).stdout).toContain("FAILED TESTS");
});

test.concurrent("generic ✕ markers name a failure", async () => {
  const res = await debugTestfail(["--json"], { stdin: "  ✕ renders the header (3 ms)\n" });
  expect(JSON.parse(res.stdout)).toMatchObject({ failures: ["renders the header (3 ms)"] });
});

test.concurrent("unrecognized input falls back to capped raw passthrough, exit 0", async () => {
  const res = await debugTestfail([], { stdin: "hello\nworld\nnothing failed\n" });
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toBe(
    "debug-testfail: no recognizable test failures — raw input (capped):\nhello\nworld\nnothing failed\n",
  );
});

test.concurrent("DEBUG_MAX_FAILURES cap truncates and marks overflow", async () => {
  const res = await debugTestfail(["--file", BUN], { env: { DEBUG_MAX_FAILURES: "1" } });
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toStartWith("FAILED TESTS (1+):\n  - add sums two numbers\n  ... (+1 more)\n");
});

test.concurrent("bad argument exits non-zero", async () => {
  const res = await debugTestfail(["--bogus"]);
  expect(res.exitCode).toBe(2);
});

test.concurrent("unreadable --file exits non-zero", async () => {
  const res = await debugTestfail(["--file", join(FX, "does-not-exist.txt")]);
  expect(res.exitCode).toBe(2);
});
