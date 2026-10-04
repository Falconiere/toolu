#!/usr/bin/env bun
/**
 * Live OpenCode host probes (#335): `bun run probe:opencode-host [--write]`.
 *
 * Resolves the pinned CLI (`TOOLU_OPENCODE_HOST_BIN`, else an npm install into
 * a version-keyed cache), runs every scenario in an isolated profile against a
 * scripted loopback provider, and compares the verdicts with the committed
 * `tools/toolu-opencode/contract/probe-results.json`. `--write` refreshes that
 * file instead. Any failure exits 1; nothing here touches the user's profile.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { runHost } from "./opencode-host/host-run.ts";
import { hostCacheDir, resolveHostBinary, type HostBinary } from "./opencode-host/install.ts";
import { contractPaths, provisionedSdkVersion, resultDrift } from "./opencode-host/results.ts";
import type { Scenario, ScenarioContext } from "./opencode-host/scenario.ts";
import { SCENARIOS } from "./opencode-host/scenarios.ts";
import {
  ContractError,
  PinSchema,
  ProbeResultsSchema,
  readJson,
  type Pin,
  type ProbeResult,
  type ProbeResults,
} from "./opencode-host/schema.ts";
import { openSession, PROBE_PLUGIN } from "./opencode-host/session.ts";

/** One warm-up session fills the shared caches and reports the host-provisioned SDK. */
export async function warmUp(ctx: ScenarioContext): Promise<string> {
  using session = openSession(ctx.cacheRoot, { localPlugins: [PROBE_PLUGIN] });
  await runHost(ctx.bin, session, ["PROBE:warm-up"]);
  const version = provisionedSdkVersion(session.sb.project);
  if (version === null)
    throw new ContractError("the host did not provision @opencode-ai/plugin into .opencode/");
  return version;
}

/** Scenarios run one at a time: they share the host's caches and must not race. */
async function runScenarios(
  ctx: ScenarioContext,
  remaining: readonly Scenario[] = SCENARIOS,
): Promise<ProbeResult[]> {
  const [scenario, ...rest] = remaining;
  if (scenario === undefined) return [];
  const { run, ...meta } = scenario;
  const observation = await run(ctx);
  process.stdout.write(`${meta.id} ${observation.verdict}\n`);
  return [{ ...meta, ...observation }, ...(await runScenarios(ctx, rest))];
}

async function liveResults(pin: Pin, host: HostBinary): Promise<ProbeResults> {
  const cacheRoot = join(hostCacheDir(pin), "run-cache");
  mkdirSync(cacheRoot, { recursive: true });
  const ctx = { bin: host.bin, cacheRoot };
  const provisioned = await warmUp(ctx);
  return {
    version: 1,
    recordedAt: new Date().toISOString().slice(0, 10),
    host: {
      cli: pin.cli.package,
      cliVersion: host.version,
      sdk: pin.sdk.package,
      provisionedSdkVersion: provisioned,
      platform: `${process.platform}-${process.arch}`,
      bun: Bun.version,
      installSource: host.installSource,
    },
    probes: await runScenarios(ctx),
  };
}

async function main(argv: string[]): Promise<number> {
  const paths = contractPaths();
  const pin = readJson(paths.pin, PinSchema);
  const live = await liveResults(pin, await resolveHostBinary(pin));
  if (argv.includes("--write")) {
    writeFileSync(paths.results, `${JSON.stringify(live, null, 2)}\n`);
    process.stdout.write(`opencode-host-probe: wrote ${paths.results}\n`);
    return 0;
  }
  const drift = resultDrift(readJson(paths.results, ProbeResultsSchema), live);
  if (drift.length > 0) {
    process.stderr.write(
      `opencode-host-probe: live results differ from ${paths.results}:\n${drift.join("\n")}\n`,
    );
    return 1;
  }
  process.stdout.write(`opencode-host-probe: ok (${live.probes.length} probes match)\n`);
  return 0;
}

if (import.meta.main) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (err: unknown) {
    if (!(err instanceof ContractError)) throw err;
    process.stderr.write(`opencode-host-probe: ${err.message}\n`);
    process.exitCode = 1;
  }
}
