/** debug-log.ts against the REAL ~3000-line / ~120KB captured log
 * (fixtures/debug/big.log: cargo+bun verbose output + git log -p). */

import { expect, test } from "bun:test";
import { join } from "node:path";
import { run, type RunOptions } from "@toolu/conformance/harness/spawn";

const SCRIPT = join(import.meta.dir, "..", "debug-log.ts");
const FX = join(import.meta.dir, "fixtures", "debug");
const BIG = join(FX, "big.log");

const debugLog = (args: string[], opts: RunOptions = {}) => run(["bun", SCRIPT, ...args], opts);
const lineCount = (out: string) => out.split("\n").filter((line) => line !== "").length;

test.concurrent("big.log: status 0 and output line count <= DEBUG_MAX_LINES (default 100)", async () => {
  const res = await debugLog(["--file", BIG]);
  expect(res.exitCode).toBe(0);
  expect(lineCount(res.stdout)).toBeLessThanOrEqual(100);
});

test.concurrent("big.log: output byte size <= DEBUG_MAX_BYTES (default 65536)", async () => {
  const res = await debugLog(["--file", BIG]);
  expect(res.exitCode).toBe(0);
  expect(Buffer.byteLength(res.stdout)).toBeLessThanOrEqual(65536);
});

test.concurrent("big.log: reports a TOTAL lines marker in the thousands", async () => {
  const res = await debugLog(["--file", BIG]);
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toMatch(/TOTAL lines: \d{4}\n$/);
});

test.concurrent("DEBUG_MAX_LINES=10 caps output to <= 10 lines", async () => {
  const res = await debugLog(["--file", BIG], { env: { DEBUG_MAX_LINES: "10" } });
  expect(res.exitCode).toBe(0);
  expect(lineCount(res.stdout)).toBeLessThanOrEqual(10);
});

test.concurrent("DEBUG_MAX_BYTES=200 drops trailing lines and says so", async () => {
  const res = await debugLog(["--file", BIG], { env: { DEBUG_MAX_BYTES: "200" } });
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toEndWith("... (output truncated to stay under 200 bytes)\n");
  const body = res.stdout.slice(0, res.stdout.lastIndexOf("..."));
  expect(Buffer.byteLength(body)).toBeLessThanOrEqual(200);
});

test.concurrent("json mode on big.log: truncated:true with a total_lines field", async () => {
  const res = await debugLog(["--json", "--file", BIG]);
  expect(res.exitCode).toBe(0);
  const out = JSON.parse(res.stdout) as {
    truncated: boolean;
    total_lines: number;
    errors: string[];
  };
  expect(out.truncated).toBe(true);
  expect(out.total_lines).toBeGreaterThanOrEqual(1000);
  expect(out.errors.length).toBeGreaterThan(0);
});

test.concurrent("dedup: identical ERROR lines collapse to one in the errors section", async () => {
  const res = await debugLog([], { stdin: "ERROR boom\nERROR boom\nERROR boom\n" });
  expect(res.exitCode).toBe(0);
  const errSection = res.stdout.slice(0, res.stdout.indexOf("TAIL "));
  expect(errSection).toBe("ERRORS/WARNINGS (1):\n  ERROR boom\n");
});

test.concurrent("dedup: timestamped repeats collapse; json control bytes stay valid JSON", async () => {
  const stdin =
    "2026-06-18T10:00:00Z error: x\n2026-06-18T10:00:01Z error: x\n\u001b[31mfail\tred\r\n";
  const res = await debugLog(["--json"], { stdin });
  expect(JSON.parse(res.stdout)).toEqual({
    errors: ["2026-06-18T10:00:00Z error: x", "\u001b[31mfail red"],
    tail: ["2026-06-18T10:00:00Z error: x", "2026-06-18T10:00:01Z error: x", "\u001b[31mfail red"],
    total_lines: 3,
    truncated: false,
  });
});

test.concurrent("empty input is valid and exits 0", async () => {
  const res = await debugLog([], { stdin: "" });
  expect(res.exitCode).toBe(0);
});

test.concurrent("bad argument exits non-zero", async () => {
  const res = await debugLog(["--bogus"]);
  expect(res.exitCode).toBe(2);
  expect(res.stderr).toStartWith("debug-log.ts: unknown arg: --bogus\nUsage: debug-log.ts");
});

test.concurrent("unreadable --file exits non-zero", async () => {
  const res = await debugLog(["--file", join(FX, "does-not-exist.log")]);
  expect(res.exitCode).toBe(2);
  expect(res.stderr).toContain("debug-log.ts: cannot read file: ");
});

test.concurrent("--file without a path exits 2", async () => {
  const res = await debugLog(["--file"]);
  expect(res.exitCode).toBe(2);
  expect(res.stderr).toBe("debug-log.ts: --file needs a path\n");
});

test.concurrent("big.log: completes without hanging (fast path, status 0)", async () => {
  const res = await debugLog(["--file", BIG], { timeoutMs: 10_000 });
  expect(res.timedOut).toBe(false);
  expect(res.exitCode).toBe(0);
});
