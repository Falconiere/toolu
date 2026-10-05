/**
 * The native measurer (#410): `cargo xtask measure` runs one command and reports
 * its tree's max RSS, CPU and wall from `getrusage(RUSAGE_CHILDREN)`. Bun cannot
 * measure this itself: on Linux a child inherits its spawner's RSS high-water
 * mark, so the measurer's own RSS is the floor (about 3 MiB on the Linux CI
 * runner, 1.3 MiB on macOS): a reading above it is exact, one at it means
 * "at most the floor".
 */
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { envOr } from "../../env.ts";
import { BenchError, MeasureReport, loadJson } from "./hook-data.ts";

/** Build the release measurer once (a no-op when current) and return its path. */
export function buildMeasurer(root: string, env: EnvPatch = process.env): string {
  const cargo = envOr("CARGO", "cargo", env);
  const built = spawnSync(cargo, ["build", "--quiet", "--locked", "--release", "-p", "xtask"], {
    cwd: root,
    encoding: "utf8",
  });
  if (built.error !== undefined) {
    throw new BenchError(`cannot run ${cargo}: ${built.error.message}`);
  }
  if (built.status !== 0) {
    throw new BenchError(`cargo build --release -p xtask failed:\n${built.stderr.trim()}`);
  }
  // Absolute: the measurer is spawned from the sandbox, not from `root`.
  return resolve(root, envOr("CARGO_TARGET_DIR", "target", env), "release", "xtask");
}

/** One spawn may take this long; the slowest Bun hook takes well under a second. */
const SPAWN_TIMEOUT_MS = 30_000;

export type Spawn = { argv: string[]; cwd: string; env: EnvPatch; stdin: string };

/**
 * Run `step` for each item strictly one after another: concurrent spawns would
 * contend and skew each other's CPU and wall.
 */
export function inSequence<T, R>(items: readonly T[], step: (item: T) => Promise<R>): Promise<R[]> {
  return items.reduce<Promise<R[]>>(
    async (done, item) => [...(await done), await step(item)],
    Promise.resolve([]),
  );
}

/** The last `count` lines of `text`. */
function tail(text: string, count = 20): string {
  return text.trimEnd().split("\n").slice(-count).join("\n");
}

/**
 * Measure one spawn of `spawn.argv`. `label` names it in errors; a command that
 * does not exit 0 is a setup error, since a broken hook must not look cheap.
 */
export async function measureOnce(
  measurer: string,
  spawn: Spawn,
  label: string,
  timeoutMs = SPAWN_TIMEOUT_MS,
): Promise<MeasureReport> {
  const dir = mkdtempSync(join(tmpdir(), "toolu-hook-bench-"));
  try {
    const out = join(dir, "report.json");
    const result = await run([measurer, "measure", "--out", out, "--", ...spawn.argv], {
      cwd: spawn.cwd,
      env: spawn.env,
      stdin: spawn.stdin,
      timeoutMs,
    });
    if (result.timedOut) {
      throw new BenchError(`${label}: timed out after ${String(timeoutMs)} ms`);
    }
    if (result.exitCode !== 0) {
      throw new BenchError(
        `${label}: measurer exited ${String(result.exitCode)}:\n${tail(result.stderr)}`,
      );
    }
    const report = loadJson(out, MeasureReport);
    if (report.exitCode !== 0) {
      const status =
        report.exitCode === null
          ? `signal ${String(report.signal)}`
          : `exit ${String(report.exitCode)}`;
      throw new BenchError(`${label}: hook ended with ${status}:\n${tail(result.stderr)}`);
    }
    return report;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
