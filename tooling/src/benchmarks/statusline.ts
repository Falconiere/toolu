#!/usr/bin/env bun
/** Measure one native statusline render against the fixed #431 budget. */
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus, platform } from "node:os";
import { dirname, join, resolve } from "node:path";
import type { MeasureReport } from "./lib/hook-data.ts";
import { buildMeasurer, measureOnce } from "./lib/hook-measurer.ts";

const ROOT = resolve(import.meta.dir, "../../..");
const RUNS = 30;
const WARMUP = 3;
const WALL_US = 4000;
const RSS_BYTES = 5 * 1024 * 1024;

function git(dir: string, ...args: string[]): void {
  const done = spawnSync("git", ["-C", dir, ...args], { encoding: "utf8" });
  if (done.status !== 0) throw new Error(`git ${args.join(" ")}: ${done.stderr}`);
}

function median(values: number[]): number {
  const sorted = values.toSorted((left, right) => left - right);
  return sorted[Math.ceil(sorted.length / 2) - 1] ?? 0;
}

function buildBinary(): void {
  const built = spawnSync(
    process.env.CARGO ?? "cargo",
    ["build", "--release", "--locked", "--bin", "toolu"],
    {
      cwd: ROOT,
      stdio: "inherit",
    },
  );
  if (built.status !== 0) throw new Error("cannot build the release toolu binary");
}

function prepareRepo(dir: string): { worktree: string; cfg: string; coldCfg: string } {
  const repo = join(dir, "repo");
  const cfg = join(dir, "cfg");
  mkdirSync(repo);
  mkdirSync(cfg);
  git(repo, "init", "-q", "-b", "main");
  git(
    repo,
    "-c",
    "user.email=bench@example.test",
    "-c",
    "user.name=Bench",
    "commit",
    "-q",
    "--allow-empty",
    "-m",
    "init",
  );
  const worktree = join(dir, "linked");
  git(repo, "worktree", "add", "-q", "-b", "bench/statusline", worktree);
  writeFileSync(join(worktree, "untracked.txt"), "statusline\n");
  const coldCfg = join(dir, "cold-cfg");
  mkdirSync(coldCfg);
  return { worktree, cfg, coldCfg };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function fixturePayload(worktree: string): string {
  const fixture: unknown = JSON.parse(
    readFileSync(join(ROOT, "fixtures/statusline/performance.json"), "utf8"),
  );
  if (!isRecord(fixture) || !isRecord(fixture.workspace))
    throw new Error("statusline performance fixture has no workspace");
  fixture.workspace.current_dir = worktree;
  return JSON.stringify(fixture);
}

type Measurer = ReturnType<typeof buildMeasurer>;
type Spawn = Parameters<typeof measureOnce>[1];

async function sampleWarm(
  measurer: Measurer,
  spawn: Spawn,
  prime: () => void,
  index = 0,
  reports: MeasureReport[] = [],
): Promise<MeasureReport[]> {
  if (index >= RUNS + WARMUP) return reports;
  prime();
  const report = await measureOnce(measurer, spawn, `statusline render ${String(index + 1)}`);
  if (index >= WARMUP) reports.push(report);
  return sampleWarm(measurer, spawn, prime, index + 1, reports);
}

async function main(): Promise<void> {
  const assert = process.argv.includes("--assert");
  buildBinary();
  const tmp = process.env.TMPDIR ?? "/tmp";
  const dir = mkdtempSync(join(tmp, "statusline-bench-"));
  try {
    const { worktree, cfg, coldCfg } = prepareRepo(dir);
    const binary = resolve(ROOT, "target/release/toolu");
    const measurer = buildMeasurer(ROOT);
    const spawn = {
      argv: [binary, "statusline", "render"],
      cwd: worktree,
      env: {
        ...process.env,
        HOME: dir,
        CLAUDE_CONFIG_DIR: cfg,
        TYPESAFE_API_KEY: "statusline-benchmark-key",
        PATH: `${dirname(binary)}:${process.env.PATH ?? ""}`,
      },
      stdin: fixturePayload(worktree),
    };
    const prime = (): void => {
      const primed = spawnSync(binary, ["statusline", "refresh-cache", worktree], {
        cwd: worktree,
        env: spawn.env,
        encoding: "utf8",
      });
      if (primed.status !== 0) throw new Error(`cannot prime statusline cache: ${primed.stderr}`);
    };
    const reports = await sampleWarm(measurer, spawn, prime);
    const wallUs = median(reports.map((report) => report.wallUs));
    const maxRssBytes = median(reports.map((report) => report.maxRssBytes));
    const cold = await measureOnce(
      measurer,
      { ...spawn, env: { ...spawn.env, CLAUDE_CONFIG_DIR: coldCfg } },
      "statusline cold first render",
    );
    const result = {
      schema: "toolu.statusline-benchmark/v1",
      platform: platform(),
      cpu: cpus()[0]?.model ?? "unknown",
      runs: RUNS,
      warmup: WARMUP,
      wallUs,
      maxRssBytes,
      coldFirstRender: { wallUs: cold.wallUs, maxRssBytes: cold.maxRssBytes },
      budget: { wallUs: WALL_US, maxRssBytes: RSS_BYTES },
    };
    const output = join(ROOT, "benchmarks/results/statusline-rust-linux-2026-10-09.json");
    writeFileSync(output, `${JSON.stringify(result, null, 2)}\n`);
    process.stdout.write(`statusline p50: ${String(wallUs)} us, ${String(maxRssBytes)} bytes\n`);
    if (assert && (wallUs > WALL_US || maxRssBytes > RSS_BYTES)) {
      process.stderr.write("statusline render exceeds the #431 budget\n");
      process.exitCode = 1;
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

await main();
