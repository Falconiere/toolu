/**
 * Tests for tooling/bats-run.sh — the parallel-aware suite runner.
 *
 * The runner's DECISIONS (job count, parallel vs serial, which flags) are
 * asserted through its print-plan mode.
 *
 * No test here runs a suite THROUGH the runner: bats does not nest — an inner
 * run inherits the outer harness's state and both misreport. The runner's
 * end-to-end proof is that `bun run test` drives the entire repo suite through
 * it, so every full run exercises it for real.
 */
import { expect, test } from "bun:test";
import { mkdirSync, symlinkSync } from "node:fs";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";

const ROOT = resolve(import.meta.dir, "../../..");
const RUNNER = resolve(ROOT, "tooling/bats-run.sh");
const NO_PARALLEL = Bun.which("parallel") === null;
const TIMEOUT_MS = 60_000;

/** A temp tree with a real two-test suite directory; returns its path. */
function fixtureSuite(sb: Sandbox): string {
  sb.write(
    "suite/fixture.bats",
    '#!/usr/bin/env bats\n@test "fixture one" { true; }\n@test "fixture two" { true; }\n',
  );
  return sb.path("suite");
}

/** A PATH with the tools bats needs but WITHOUT GNU parallel. */
function pathWithoutParallel(sb: Sandbox): string {
  const bin = sb.path("bin");
  mkdirSync(bin, { recursive: true });
  const tools = [
    "bats",
    "bash",
    "env",
    "dirname",
    "basename",
    "mkdir",
    "rm",
    "cat",
    "sed",
    "grep",
    "cut",
    "sort",
    "tr",
    "awk",
    "getconf",
    "uname",
    "mktemp",
    "find",
    "flock",
    "realpath",
    "readlink",
    "tput",
  ];
  for (const tool of tools) {
    const source = Bun.which(tool);
    if (source !== null) {
      symlinkSync(source, `${bin}/${tool}`);
    }
  }
  return bin;
}

/**
 * Run the real runner in print-plan mode from the repo root (its default paths
 * are relative), merging stdout and stderr like bats `$output`.
 */
async function plan(
  env: Record<string, string | undefined>,
  args: string[] = [],
): Promise<{ exitCode: number; output: string }> {
  const res = await run(["bash", RUNNER, ...args], {
    cwd: ROOT,
    env: { BATS_RUN_PRINT_PLAN: "1", ...env },
  });
  return { exitCode: res.exitCode, output: res.stdout + res.stderr };
}

test.concurrent.skipIf(NO_PARALLEL)(
  "plans a parallel run when GNU parallel is available",
  async () => {
    using sb = createSandbox();
    const res = await plan({ BATS_JOBS: "4" }, [fixtureSuite(sb)]);
    expect(res.exitCode).toBe(0);
    expect(res.output).toContain("--jobs 4");
    expect(res.output).toContain("--no-parallelize-within-files");
  },
  TIMEOUT_MS,
);

test.concurrent.skipIf(NO_PARALLEL)(
  "announces what it is doing on stderr",
  async () => {
    using sb = createSandbox();
    const res = await plan({}, [fixtureSuite(sb)]);
    expect(res.output).toContain("files in parallel");
    expect(res.output).toContain("tests within a file serial");
  },
  TIMEOUT_MS,
);

test.concurrent(
  "BATS_JOBS=1 plans a serial run",
  async () => {
    using sb = createSandbox();
    const res = await plan({ BATS_JOBS: "1" }, [fixtureSuite(sb)]);
    expect(res.exitCode).toBe(0);
    expect(res.output).not.toContain("--jobs");
  },
  TIMEOUT_MS,
);

test.concurrent(
  "a non-numeric BATS_JOBS plans a serial run rather than passing it through",
  async () => {
    using sb = createSandbox();
    const res = await plan({ BATS_JOBS: "many" }, [fixtureSuite(sb)]);
    expect(res.exitCode).toBe(0);
    expect(res.output).not.toContain("--jobs");
    expect(res.output).not.toContain("many");
  },
  TIMEOUT_MS,
);

test.concurrent(
  "falls back to serial with a hint when parallel is missing",
  async () => {
    using sb = createSandbox();
    const suite = fixtureSuite(sb);
    const bash = Bun.which("bash");
    expect(bash).not.toBeNull();
    const res = await run([bash ?? "bash", RUNNER, suite], {
      cwd: sb.project,
      env: { PATH: pathWithoutParallel(sb), BATS_RUN_PRINT_PLAN: "1" },
    });
    const output = res.stdout + res.stderr;
    expect(res.exitCode).toBe(0);
    expect(output).toContain("GNU parallel not found");
    expect(output).toContain("running serially");
    expect(output).not.toContain("--jobs");
  },
  TIMEOUT_MS,
);

test.concurrent(
  "defaults to the repo suite paths when none are given",
  async () => {
    const res = await plan({ BATS_JOBS: "1" });
    expect(res.output).toContain("bats -r plugins tooling packages tools");
  },
  TIMEOUT_MS,
);

test.concurrent(
  "the default covers every directory holding suites",
  async () => {
    // A suite directory missing from the default is a suite nobody runs locally.
    const res = await plan({ BATS_JOBS: "1" });
    for (const dir of ["plugins", "tooling", "packages", "tools"]) {
      expect(res.output).toContain(dir);
    }
  },
  TIMEOUT_MS,
);

test.concurrent(
  "a path with no suites fails loudly rather than confusingly",
  async () => {
    using sb = createSandbox();
    mkdirSync(sb.path("empty"), { recursive: true });
    const res = await plan({ BATS_JOBS: "1" }, [sb.path("empty")]);
    expect(res.exitCode).not.toBe(0);
    expect(res.output).toContain("test discovery is broken");
  },
  TIMEOUT_MS,
);

test.concurrent(
  "the given paths are what it plans to run",
  async () => {
    using sb = createSandbox();
    const suite = fixtureSuite(sb);
    const res = await plan({ BATS_JOBS: "1" }, [suite]);
    expect(res.output).toContain(suite);
  },
  TIMEOUT_MS,
);
