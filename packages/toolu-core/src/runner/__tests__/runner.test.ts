import { expect, test } from "bun:test";
import { mkdir, mkdtemp, realpath } from "node:fs/promises";
import { join } from "node:path";
import { createBunBashRunner } from "../runner.ts";

const runner = createBunBashRunner();

test("runner timeout yields runtime_failure-capable spawn result not ok timeout", async () => {
  const result = await runner.run({
    argv: ["bash", "-c", "sleep 5"],
    cwd: process.cwd(),
    env: {},
    stdin: "",
    deadlineMs: 200,
    maxStdoutBytes: 65536,
  });
  expect(result.ok).toBe(false);
  if (!result.ok) {
    expect(result.code).toBe("timeout");
  }
});

test("runner exit 2 returns ok true exitCode 2 for bridge mapping", async () => {
  const result = await runner.run({
    argv: ["bash", "-c", "echo blocked 1>&2; exit 2"],
    cwd: process.cwd(),
    env: {},
    stdin: "",
    deadlineMs: 5000,
    maxStdoutBytes: 65536,
  });
  expect(result.ok).toBe(true);
  if (result.ok) {
    expect(result.exitCode).toBe(2);
    expect(result.stderr).toContain("blocked");
  }
});

test("runner spawn failure missing binary yields spawn code", async () => {
  const result = await runner.run({
    argv: ["toolu-nonexistent-binary-210"],
    cwd: process.cwd(),
    env: {},
    stdin: "",
    deadlineMs: 5000,
    maxStdoutBytes: 65536,
  });
  expect(result.ok).toBe(false);
  if (!result.ok) {
    expect(result.code).toBe("spawn");
  }
});

test("runner path with spaces cwd runs echo ok", async () => {
  const base = process.env.TMPDIR ?? "/tmp";
  const cwd = await mkdtemp(join(base, "toolu spaces "));
  await mkdir(cwd, { recursive: true });
  const result = await runner.run({
    argv: ["bash", "-c", "pwd"],
    cwd,
    env: {},
    stdin: "",
    deadlineMs: 5000,
    maxStdoutBytes: 65536,
  });
  expect(result.ok).toBe(true);
  if (result.ok) {
    // macOS exposes TMPDIR as /var/... while pwd resolves /private/var/...
    expect(result.stdout.trim()).toBe(await realpath(cwd));
  }
});

test("runner malformed handled at bridge not runner", async () => {
  const result = await runner.run({
    argv: ["bash", "-c", "echo not-json"],
    cwd: process.cwd(),
    env: {},
    stdin: "",
    deadlineMs: 5000,
    maxStdoutBytes: 65536,
  });
  expect(result.ok).toBe(true);
});
