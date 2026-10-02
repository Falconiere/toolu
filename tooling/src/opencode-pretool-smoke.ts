#!/usr/bin/env bun
/** Live OpenCode 1.18.34 pre-tool enforcement smoke for issue #338. */
import { existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { hostCacheDir, resolveHostBinary } from "./opencode-host/install.ts";
import { contractPaths } from "./opencode-host/results.ts";
import { PRETOOL_SCENARIOS, type PretoolScenario } from "./opencode-host/scenarios-pretool.ts";
import { ContractError, PinSchema, readJson, type Pin } from "./opencode-host/schema.ts";

/** The live-only smoke tolerates shared-host startup load while still checking the real CLI pin. */
async function smokeHost(pin: Pin): Promise<{ bin: string; version: string }> {
  const bin =
    process.env.TOOLU_OPENCODE_HOST_BIN ??
    join(hostCacheDir(pin), "cli/node_modules/.bin/opencode");
  if (!existsSync(bin)) return resolveHostBinary(pin);
  const res = await run([bin, "--version"], { timeoutMs: 90_000 });
  const version = res.stdout.trim().replace(/^v/, "");
  if (res.exitCode !== 0 || res.timedOut || version !== pin.cli.version) {
    throw new ContractError(
      `pinned OpenCode CLI expected ${pin.cli.version}, got ${version || `exit ${res.exitCode}`}`,
    );
  }
  return { bin, version };
}

async function runAll(
  ctx: { bin: string; cacheRoot: string },
  remaining: readonly PretoolScenario[],
): Promise<number> {
  const [scenario, ...rest] = remaining;
  if (scenario === undefined) return 0;
  const result = await scenario.run(ctx);
  process.stdout.write(
    `${scenario.id} ${result.pass ? "pass" : "FAIL"} ${JSON.stringify(result.observed)}\n`,
  );
  return (result.pass ? 0 : 1) + (await runAll(ctx, rest));
}

async function main(): Promise<number> {
  const pin = readJson(contractPaths().pin, PinSchema);
  const host = await smokeHost(pin);
  const cacheRoot = join(hostCacheDir(pin), "run-cache");
  mkdirSync(cacheRoot, { recursive: true });
  const selected =
    process.argv[2] === undefined
      ? PRETOOL_SCENARIOS
      : PRETOOL_SCENARIOS.filter((scenario) => scenario.id === process.argv[2]);
  if (selected.length === 0)
    throw new ContractError(`unknown pre-tool scenario: ${process.argv[2]}`);
  const failed = await runAll({ bin: host.bin, cacheRoot }, selected);
  process.stdout.write(
    `opencode-pretool-smoke: ${selected.length - failed}/${selected.length} pass on opencode-ai@${host.version}\n`,
  );
  return failed === 0 ? 0 : 1;
}

if (import.meta.main) process.exitCode = await main();
