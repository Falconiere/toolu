/** debug-stack.ts against REAL captured node + cargo backtraces
 * (see fixtures/debug/PROVENANCE.md). No synthetic stack output. */

import { expect, test } from "bun:test";
import { join } from "node:path";
import { run, type RunOptions } from "@toolu/conformance/harness/spawn";

const SCRIPT = join(import.meta.dir, "..", "debug-stack.ts");
const FX = join(import.meta.dir, "fixtures", "debug");
const JS = join(FX, "stacktrace.txt");
const RUST = join(FX, "rust-panic.txt");

const debugStack = (args: string[], opts: RunOptions = {}) => run(["bun", SCRIPT, ...args], opts);

test.concurrent("js: surfaces inner/middle/outer app frames with a throw.mjs location", async () => {
  const res = await debugStack(["--file", JS]);
  expect(res.exitCode).toBe(0);
  for (const frame of ["inner", "middle", "outer", "throw.mjs:1:25"]) {
    expect(res.stdout).toContain(frame);
  }
});

test.concurrent("js: node:internal frames are collapsed, not listed as app frames", async () => {
  const res = await debugStack(["--file", JS]);
  expect(res.exitCode).toBe(0);
  expect(res.stdout).not.toContain("node:internal");
  expect(res.stdout).toContain("framework/runtime frames collapsed");
});

test.concurrent("rust: surfaces level_three/level_two/level_one app frames", async () => {
  const res = await debugStack(["--file", RUST]);
  expect(res.exitCode).toBe(0);
  for (const frame of ["level_three", "level_two", "level_one"]) {
    expect(res.stdout).toContain(frame);
  }
});

test.concurrent("rust: core::panicking and __rustc frames are collapsed, not listed", async () => {
  const res = await debugStack(["--file", RUST]);
  expect(res.exitCode).toBe(0);
  expect(res.stdout).not.toContain("core::panicking");
  expect(res.stdout).not.toContain("__rustc");
  expect(res.stdout).toContain("framework/runtime frames collapsed");
});

test.concurrent("language-agnostic: both fixtures produce an APP FRAMES section", async () => {
  expect((await debugStack(["--file", JS])).stdout).toContain("APP FRAMES");
  expect((await debugStack(["--file", RUST])).stdout).toContain("APP FRAMES");
});

test.concurrent("json mode emits recognized:true with a non-empty app_frames", async () => {
  const res = await debugStack(["--json", "--file", RUST]);
  expect(res.exitCode).toBe(0);
  const out = JSON.parse(res.stdout) as { app_frames: string[]; noise_frames: number; recognized: boolean };
  expect(out.recognized).toBe(true);
  expect(out.app_frames[0]).toStartWith("panicker::level_three");
  expect(out.noise_frames).toBeGreaterThan(0);
});

test.concurrent("DEBUG_MAX_FRAMES cap truncates and marks overflow", async () => {
  const res = await debugStack(["--file", RUST], { env: { DEBUG_MAX_FRAMES: "1" } });
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toMatch(/^APP FRAMES \(\d+\+\):\n {2}- panicker::level_three.*\n {2}\.\.\. \(\+\d+ more\)\n/);
});

test.concurrent("unrecognized input falls back to capped raw passthrough, exit 0", async () => {
  const res = await debugStack([], { stdin: "just some text\n" });
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toBe(
    "debug-stack: no recognizable stack frames — raw input (capped):\njust some text\n",
  );
});

test.concurrent("raw passthrough is capped by DEBUG_MAX_LINES", async () => {
  const res = await debugStack([], { stdin: "a\nb\nc\n", env: { DEBUG_MAX_LINES: "1" } });
  expect(res.stdout).toBe(
    "debug-stack: no recognizable stack frames — raw input (capped):\na\n... (+2 more lines)\n",
  );
});

test.concurrent("bad argument exits non-zero", async () => {
  const res = await debugStack(["--bogus"]);
  expect(res.exitCode).toBe(2);
});

test.concurrent("unreadable --file exits non-zero", async () => {
  const res = await debugStack(["--file", join(FX, "does-not-exist.txt")]);
  expect(res.exitCode).toBe(2);
});
