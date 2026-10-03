#!/usr/bin/env bun
/** Live OpenCode 1.18.34 ast-grep smoke for issue #347. */
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { hostCacheDir, resolveHostBinary } from "./opencode-host/install.ts";
import { runPretoolScenarios } from "./opencode-host/pretool-shared.ts";
import { contractPaths } from "./opencode-host/results.ts";
import { AST_GREP_SCENARIOS } from "./opencode-host/scenarios-ast-grep-smoke.ts";
import { ContractError, PinSchema, readJson } from "./opencode-host/schema.ts";

async function main(): Promise<number> {
  const pin = readJson(contractPaths().pin, PinSchema);
  const host = await resolveHostBinary(pin, process.env, 90_000);
  const cacheRoot = join(hostCacheDir(pin), "run-cache");
  mkdirSync(cacheRoot, { recursive: true });
  const selected =
    process.argv[2] === undefined
      ? AST_GREP_SCENARIOS
      : AST_GREP_SCENARIOS.filter((scenario) => scenario.id === process.argv[2]);
  if (selected.length === 0)
    throw new ContractError(`unknown post-tool scenario: ${process.argv[2]}`);
  const failed = await runPretoolScenarios({ bin: host.bin, cacheRoot }, selected);
  process.stdout.write(
    `opencode-ast-grep-smoke: ${selected.length - failed}/${selected.length} pass on opencode-ai@${host.version}\n`,
  );
  return failed === 0 ? 0 : 1;
}

if (import.meta.main) process.exitCode = await main();
