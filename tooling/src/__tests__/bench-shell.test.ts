/**
 * `bench:shell` (#284 AC-1): the probe bundles are built by the plugin bundle
 * pipeline and run from a directory with no node_modules. Every runtime export
 * of `@toolu/core/shell` adds at most 200,000 bytes unminified, and so does
 * `analyzeShell` plus `@toolu/core/shell/writes`. Cold start uses the minified
 * production format. Wall-clock numbers are machine-bound, so this suite
 * checks their presence, not their values; CI also runs
 * `bun run bench:shell --assert` to check timing on that runner.
 */
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
import { BUDGET, fixtureCommands, overBudget, type ShellBench } from "../bench-shell.ts";

const ROOT = resolve(import.meta.dir, "../../..");
const SCRIPT = resolve(ROOT, "tooling/src/bench-shell.ts");

const Positive = z.number().positive();
const Report = z.object({
  machine: z.object({
    bun: z.string(),
    platform: z.string(),
    arch: z.string(),
    cpu: z.string(),
    date: z.string(),
  }),
  bundle: z.object({
    emptyBytes: Positive,
    deltaBytes: Positive,
    writesDeltaBytes: Positive,
    togetherDeltaBytes: Positive,
  }),
  coldStart: z.object({
    runs: Positive,
    emptyP50: Positive,
    shellP50: Positive,
    deltaP50: z.number(),
    emptyP90: Positive,
    shellP90: Positive,
  }),
  parse: z.object({
    commands: Positive,
    samples: Positive,
    p50Us: Positive,
    p99Us: Positive,
    maxUs: Positive,
  }),
  probeOutput: z.string(),
});

test("each entry fits 200,000 bytes, both together 205,000, and they run without node_modules", async () => {
  const res = await run([process.execPath, SCRIPT, "--json", "--runs", "3", "--rounds", "1"], {
    cwd: ROOT,
    timeoutMs: 120_000,
  });
  expect(res.exitCode).toBe(0);
  const report = Report.parse(JSON.parse(res.stdout));
  expect(report.bundle.deltaBytes).toBeLessThanOrEqual(BUDGET.bundleBytes);
  expect(report.bundle.writesDeltaBytes).toBeLessThanOrEqual(BUDGET.writesBundleBytes);
  expect(report.bundle.togetherDeltaBytes).toBeLessThanOrEqual(BUDGET.togetherBundleBytes);
  expect(report.bundle.togetherDeltaBytes).toBeGreaterThan(report.bundle.writesDeltaBytes);
  expect(JSON.parse(report.probeOutput)).toMatchObject({ push: "yes", destination: "feat/x" });
  expect(report.parse.commands).toBe(fixtureCommands().length);
  expect(report.coldStart.runs).toBe(3);
});

test.concurrent("the size budgets are the product owner's numbers", () => {
  expect(BUDGET).toMatchObject({
    bundleBytes: 200_000,
    writesBundleBytes: 200_000,
    togetherBundleBytes: 205_000,
  });
});

test.concurrent("the fixture set covers both fixture files", () => {
  expect(fixtureCommands().length).toBeGreaterThanOrEqual(186 + 49);
});

test.concurrent("overBudget names every budget a report exceeds", () => {
  const within: ShellBench = {
    machine: { bun: "1.4.2", platform: "linux", arch: "x64", cpu: "cpu", date: "2026-09-28" },
    bundle: {
      emptyBytes: 73,
      deltaBytes: 197_434,
      writesDeltaBytes: 199_180,
      togetherDeltaBytes: 203_159,
    },
    coldStart: { runs: 40, emptyP50: 5, shellP50: 8, deltaP50: 3, emptyP90: 6, shellP90: 9 },
    parse: { commands: 235, samples: 4700, p50Us: 3, p99Us: 40, maxUs: 900 },
    probeOutput: "{}",
  };
  expect(overBudget(within)).toEqual([]);
  const over = {
    ...within,
    bundle: {
      ...within.bundle,
      deltaBytes: 200_001,
      writesDeltaBytes: 200_001,
      togetherDeltaBytes: 205_001,
    },
    coldStart: { ...within.coldStart, deltaP50: 5.5 },
    parse: { ...within.parse, p99Us: 120 },
  };
  expect(overBudget(over).map((problem) => problem.split(" ")[0])).toEqual([
    "bundle",
    "writes",
    "together",
    "cold",
    "parse",
  ]);
});

test.concurrent("unknown options and bad counts are rejected", async () => {
  const unknown = await run([process.execPath, SCRIPT, "--bogus"], { cwd: ROOT });
  expect(unknown.exitCode).toBe(1);
  expect(unknown.stderr).toContain("unknown option --bogus");
  const bad = await run([process.execPath, SCRIPT, "--runs", "0"], { cwd: ROOT });
  expect(bad.exitCode).toBe(1);
  expect(bad.stderr).toContain("--runs needs a positive integer");
  const missing = await run([process.execPath, SCRIPT, "--rounds"], { cwd: ROOT });
  expect(missing.exitCode).toBe(1);
  expect(missing.stderr).toContain("--rounds needs a value");
  // A flag after a value-taking flag is never read for that flag's value.
  const swallowed = await run([process.execPath, SCRIPT, "--runs", "--bogus"], { cwd: ROOT });
  expect(swallowed.exitCode).toBe(1);
  expect(swallowed.stderr).toContain("--runs needs a value");
  const stray = await run([process.execPath, SCRIPT, "--json", "extra"], { cwd: ROOT });
  expect(stray.exitCode).toBe(1);
  expect(stray.stderr).toContain("unknown option extra");
});
