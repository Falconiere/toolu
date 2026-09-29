#!/usr/bin/env bun
/**
 * write-state.sh — writes the push-review state file the toolu push-review
 * gate validates (`<repo>/<host dir>/tmp/push-review/<branch-slug>.json`,
 * schema v2). TypeScript port of the bash helper (#269): same flags, messages
 * and exit statuses. The SessionStart hook publishes this bundle at
 * `<config root>/toolu-review/write-state.sh`.
 *
 * Usage: write-state.sh --findings-count N [--reviewers JSON] [--findings JSON]
 *                       [--repo PATH] [--branch NAME] [--reviewed-files a,b]
 * Prints the state file path on success.
 */
import { runCli, writeStdout } from "@toolu/core/cli";
import { parseArgs } from "./write-state/args.ts";
import { writeReviewState } from "./write-state/review-state.ts";

async function main(): Promise<number> {
  const file = writeReviewState(parseArgs(process.argv.slice(2)), process.env, process.cwd());
  await writeStdout(`${file}\n`);
  return 0;
}

await runCli(main);
