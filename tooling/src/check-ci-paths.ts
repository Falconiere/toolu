#!/usr/bin/env bun
/**
 * CI path-group check (#458): `bun run check:ci-paths [--github <dir>]`.
 * Reads `<dir>/ci-paths.json` and `<dir>/workflows/*.yml` (default: this
 * checkout's `.github`) and this checkout's tracked files, and exits 1 with one
 * line per violation (see ci-paths/check.ts), 0 when consistent.
 */
import { spawnSync } from "node:child_process";
import { join, resolve } from "node:path";
import { checkCiPaths } from "./ci-paths/check.ts";
import { CiPathsError, loadCiPaths } from "./ci-paths/config.ts";
import { readWorkflows } from "./ci-paths/workflow.ts";

const ROOT = resolve(import.meta.dir, "../..");

function githubDir(argv: string[]): string {
  const at = argv.indexOf("--github");
  const value = at === -1 ? undefined : argv[at + 1];
  return value === undefined ? join(ROOT, ".github") : resolve(value);
}

function trackedFiles(): string[] {
  const res = spawnSync("git", ["-C", ROOT, "ls-files", "-z"], { encoding: "utf8" });
  if (res.status !== 0) throw new Error(`git ls-files failed: ${res.stderr.trim()}`);
  return res.stdout.split("\0").filter((path) => path !== "");
}

function main(argv: string[]): number {
  const dir = githubDir(argv);
  let problems: string[];
  try {
    const config = loadCiPaths(join(dir, "ci-paths.json"));
    const { workflows, errors } = readWorkflows(join(dir, "workflows"));
    problems = [...errors, ...checkCiPaths(config, workflows, trackedFiles())];
  } catch (error) {
    if (!(error instanceof CiPathsError)) throw error;
    problems = [error.message];
  }
  if (problems.length === 0) {
    process.stdout.write("check:ci-paths: workflows and path groups agree\n");
    return 0;
  }
  process.stderr.write(`${problems.map((line) => `check:ci-paths: ${line}`).join("\n")}\n`);
  return 1;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
