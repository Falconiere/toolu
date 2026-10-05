/**
 * `bun run bench:hooks` end to end (#410): the real bench over the repository's
 * hook entries, measured by the native `cargo xtask measure`, so these tests
 * need the Rust toolchain. `test:unit` skips `*.native.test.ts`; the hook-bench
 * CI job runs this file. A real executable shell script stands in for the
 * `toolu` binary no entry is ported to yet. Tests run one at a time: concurrent
 * spawns would skew each other's CPU.
 */
import { expect, test } from "bun:test";
import { chmodSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type RunResult } from "@toolu/conformance/harness/spawn";
import { BenchError, HookResult, Payloads, loadJson } from "../lib/hook-data.ts";
import { buildMeasurer, measureOnce } from "../lib/hook-measurer.ts";
import { discoverEntries } from "../lib/hook-entries.ts";

const ROOT = resolve(import.meta.dir, "../../../..");
const BENCH = join(ROOT, "tooling/src/benchmarks/hook-resources.ts");
const TIMEOUT = 180_000;

function bench(args: string[], env: Record<string, string | undefined> = {}): Promise<RunResult> {
  return run([process.execPath, BENCH, ...args], {
    cwd: ROOT,
    env: { TOOLU_IMPL: undefined, TOOLU_RUST_BIN_DIR: undefined, ...env },
    timeoutMs: TIMEOUT,
  });
}

/** A `toolu` stand-in in its own bin directory: builtins only, so it costs ~1.5 ms CPU. */
function standIn(sb: Sandbox, body: string): string {
  const file = sb.write("bin/toolu", `#!/bin/sh\n${body}\nprintf '{}\\n'\n`);
  chmodSync(file, 0o755);
  return sb.path("bin");
}

/**
 * Fifty `git` spawns per call: about 50 ms CPU on the Linux CI runner and far more
 * on macOS, well above a budget calibrated from the git-free stand-in.
 */
const GIT_FIFTY_TIMES =
  'i=0; while [ "$i" -lt 50 ]; do git rev-parse --show-toplevel >/dev/null 2>&1; i=$((i+1)); done';

function ported(sb: Sandbox, entries: string[], budgets: object): string[] {
  const manifest = sb.write("rust-ported.json", { entries });
  const budgetFile = sb.write("hook-budgets.json", {
    version: 1,
    percentile: 50,
    entries: budgets,
  });
  return ["--assert", "--manifest", manifest, "--budgets", budgetFile];
}

test(
  "every hook entry is reported with p50/p90 RSS, CPU and wall",
  async () => {
    using sb = createSandbox();
    const out = sb.path("result.json");
    const result = await bench(["--runs", "1", "--warmup", "0", "--out", out]);
    expect(result.exitCode, result.stderr).toBe(0);
    const doc = HookResult.parse(JSON.parse(sb.read("result.json")));
    const ids = discoverEntries(join(ROOT, "plugins")).map((e) => e.id);
    expect(doc.entries.map((e) => e.entry)).toEqual(ids);
    for (const row of doc.entries) {
      expect(row.implementation, row.entry).toBe("bun");
      for (const metric of [row.maxRssBytes, row.cpuUs, row.wallUs]) {
        expect(metric.p50, row.entry).toBeGreaterThan(0);
        expect(metric.p90, row.entry).toBeGreaterThanOrEqual(metric.p50);
      }
    }
    expect(doc.provenance.floor.maxRssBytes).toBeGreaterThan(0);
    for (const id of ids) expect(result.stdout).toContain(`| ${id} |`);
  },
  TIMEOUT,
);

test("payloads must cover exactly the discovered entries", async () => {
  using sb = createSandbox();
  const committed = loadJson(join(ROOT, "benchmarks/cases/hooks/payloads.json"), Payloads);
  const { "toolu/pre-tools": dropped, ...rest } = committed.entries;
  expect(dropped).toBeDefined();
  const file = sb.write("payloads.json", {
    version: 1,
    entries: { ...rest, "toolu/nope": { stdin: {} } },
  });
  const result = await bench(["--payloads", file]);
  expect(result.exitCode).toBe(2);
  expect(result.stderr).toContain("no payload for: toolu/pre-tools");
  expect(result.stderr).toContain("payloads for unknown entries: toolu/nope");
});

test(
  "a ported entry that spawns git on every call fails on CPU; without git it passes",
  async () => {
    using sb = createSandbox();
    const only = ["--only", "toolu/pre-tools", "--runs", "5", "--warmup", "1"];
    // Calibrate on this machine: spawning costs ~1.5 ms CPU on Linux and ~13 ms on
    // macOS, so the budget is twice the git-free stand-in's p90 plus 1 ms.
    const out = sb.path("calibration.json");
    const calibration = await bench([...only, "--out", out], {
      TOOLU_IMPL: "rust:toolu/pre-tools",
      TOOLU_RUST_BIN_DIR: standIn(sb, ":"),
    });
    expect(calibration.exitCode, calibration.stderr).toBe(0);
    const p90 = HookResult.parse(JSON.parse(sb.read("calibration.json"))).entries[0]?.cpuUs.p90;
    expect(p90).toBeGreaterThan(0);
    const cpuMs = Math.ceil(((p90 ?? 0) / 1000) * 2) + 1;
    const flags = [
      ...only,
      ...ported(sb, ["toolu/pre-tools"], { "toolu/pre-tools": { maxRssMiB: 1024, cpuMs } }),
    ];
    const slow = await bench(flags, { TOOLU_RUST_BIN_DIR: standIn(sb, GIT_FIFTY_TIMES) });
    expect(slow.exitCode, slow.stderr).toBe(1);
    expect(slow.stderr).toMatch(
      new RegExp(
        `hook-bench: toolu/pre-tools \\[rust\\]: cpu p50 \\d+\\.\\d ms > budget ${String(cpuMs)} ms`,
      ),
    );
    expect(slow.stdout).toContain("| toolu/pre-tools | PreToolUse | rust |");
    const fast = await bench(flags, { TOOLU_RUST_BIN_DIR: standIn(sb, ":") });
    expect(fast.exitCode, fast.stderr).toBe(0);
  },
  TIMEOUT,
);

test(
  "a ported entry without a budget fails; an empty manifest asserts nothing",
  async () => {
    using sb = createSandbox();
    const bin = standIn(sb, ":");
    const unbudgeted = ported(sb, ["toolu/agent-tier"], {
      "toolu/pre-tools": { maxRssMiB: 8, cpuMs: 5 },
    });
    const missing = await bench(
      ["--only", "toolu/agent-tier", "--runs", "1", "--warmup", "0", ...unbudgeted],
      {
        TOOLU_RUST_BIN_DIR: bin,
      },
    );
    expect(missing.exitCode, missing.stderr).toBe(1);
    expect(missing.stderr).toContain("toolu/agent-tier [rust]: no budget in");
    const none = await bench([
      "--only",
      "toolu/agent-tier",
      "--runs",
      "1",
      "--warmup",
      "0",
      ...ported(sb, [], {}),
    ]);
    expect(none.exitCode, none.stderr).toBe(0);
    expect(none.stdout).toContain("hook-bench: no ported entries; nothing to assert");
  },
  TIMEOUT,
);

test(
  "a hook that fails is a setup error, not a cheap measurement",
  async () => {
    using sb = createSandbox();
    const flags = ["--only", "toolu/pre-tools", "--runs", "1", "--warmup", "0"];
    const result = await bench(flags, {
      TOOLU_IMPL: "rust:toolu/pre-tools",
      TOOLU_RUST_BIN_DIR: standIn(sb, "echo broken >&2; exit 3"),
    });
    expect(result.exitCode).toBe(2);
    expect(result.stderr).toContain("toolu/pre-tools [rust]: hook ended with exit 3");
    expect(result.stderr).toContain("broken");
  },
  TIMEOUT,
);

test("a spawn that outlives its timeout is a setup error that says so", async () => {
  using sb = createSandbox();
  const measurer = buildMeasurer(ROOT);
  const spawn = { argv: ["sleep", "5"], cwd: sb.project, env: {}, stdin: "" };
  let caught: unknown;
  try {
    await measureOnce(measurer, spawn, "toolu/slow [rust]", 300);
  } catch (error) {
    caught = error;
  }
  expect(caught).toBeInstanceOf(BenchError);
  expect(String(caught)).toContain("toolu/slow [rust]: timed out after 300 ms");
});
