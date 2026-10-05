#!/usr/bin/env bun
/**
 * The `changes` job (#458): `bun run tooling/src/ci-changes.ts` classifies the
 * event's diff with `.github/ci-paths.json` and appends one `<group>=true|false`
 * line per group, plus `changed`, to `$GITHUB_OUTPUT`.
 *
 * Fails open: an unreadable event, a dispatch, a push without a previous
 * commit, an empty diff or a `git diff` error turns every group on. It exits
 * non-zero only when the data file is invalid or the output cannot be written,
 * so the required aggregates fail instead of passing on missing outputs.
 * `CI_CHANGES_ROOT` points it at another checkout.
 */
import { appendFileSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { allOn, classify, type Classification } from "./ci-paths/classify.ts";
import { type CiPaths, CiPathsError, loadRepoCiPaths } from "./ci-paths/config.ts";
import { DiffError, readDiff, resolveRange } from "./ci-paths/diff.ts";
import { envOr } from "./env.ts";

function readEvent(path: string): unknown {
  if (path === "") return null;
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
}

function decide(root: string, config: CiPaths): Classification {
  const eventName = envOr("GITHUB_EVENT_NAME", "");
  const range = resolveRange(eventName, readEvent(envOr("GITHUB_EVENT_PATH", "")));
  if (range.kind === "all") return allOn(config, range.reason);
  try {
    return classify(config, readDiff(root, range.spec, config));
  } catch (error) {
    if (error instanceof DiffError) return allOn(config, `the diff failed: ${error.message}`);
    throw error;
  }
}

function main(): number {
  const root = resolve(envOr("CI_CHANGES_ROOT", process.cwd()));
  const output = envOr("GITHUB_OUTPUT", "");
  if (output === "") {
    process.stderr.write("ci-changes: GITHUB_OUTPUT is not set\n");
    return 2;
  }
  let config: CiPaths;
  try {
    config = loadRepoCiPaths(root);
  } catch (error) {
    if (!(error instanceof CiPathsError)) throw error;
    process.stderr.write(`ci-changes: ${error.message}\n`);
    return 1;
  }
  const result = decide(root, config);
  const lines = Object.entries(result.outputs).map(([name, on]) => `${name}=${String(on)}`);
  appendFileSync(output, `${lines.join("\n")}\n`);
  process.stdout.write(`${[...result.reasons, ...lines].join("\n")}\n`);
  return 0;
}

if (import.meta.main) process.exit(main());
