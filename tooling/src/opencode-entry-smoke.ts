#!/usr/bin/env bun
/**
 * Live OpenCode entry smoke (#336), plugin startup (#342), paths and helper
 * environment (#343), surface install and discovery (#345), leaf plugins,
 * pr-babysit's controller and OpenCode fixer (#357), and CLI management (#360):
 * `bun run smoke:opencode-entry [<scenario id>…]`; ids narrow the run.
 *
 * Resolves the pinned CLI the same way as `probe:opencode-host`, packs
 * `@toolu/opencode` into a temp directory, and runs every entry scenario in an
 * isolated profile against the scripted loopback provider. The first run needs
 * the network: the host provisions its SDK and installs the tarball's registry
 * dependencies. Any failed expectation, or any host error, exits 1.
 */
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { hostCacheDir, resolveHostBinary } from "./opencode-host/install.ts";
import { contractPaths } from "./opencode-host/results.ts";
import {
  ENTRY_SCENARIOS,
  packTarball,
  type EntryContext,
  type EntryScenario,
} from "./opencode-host/scenarios-entry.ts";
import { ContractError, PinSchema, readJson } from "./opencode-host/schema.ts";
import { INSTALL_SCENARIOS } from "./opencode-host/scenarios-install.ts";
import { PATH_SCENARIOS } from "./opencode-host/scenarios-paths.ts";
import { STARTUP_SCENARIOS } from "./opencode-host/scenarios-startup.ts";
import { BROWSER_SCENARIOS } from "./opencode-host/scenarios-browser.ts";
import { EXA_SCENARIOS } from "./opencode-host/scenarios-exa.ts";
import { BABYSIT_SCENARIOS } from "./opencode-host/scenarios-babysit.ts";
import { CLI_SCENARIOS } from "./opencode-host/scenarios-cli.ts";

const ALL_SCENARIOS = [
  ...ENTRY_SCENARIOS,
  ...STARTUP_SCENARIOS,
  ...PATH_SCENARIOS,
  ...INSTALL_SCENARIOS,
  ...BROWSER_SCENARIOS,
  ...EXA_SCENARIOS,
  ...BABYSIT_SCENARIOS,
  ...CLI_SCENARIOS,
];

/** The scenarios named in `ids`, or every one; an unknown id is an error, not an empty run. */
export function chosen(ids: readonly string[]): EntryScenario[] {
  const unknown = ids.filter((id) => !ALL_SCENARIOS.some((scenario) => scenario.id === id));
  if (unknown.length > 0) throw new ContractError(`unknown scenario: ${unknown.join(", ")}`);
  return ids.length === 0 ? ALL_SCENARIOS : ALL_SCENARIOS.filter((s) => ids.includes(s.id));
}

/** Scenarios run one at a time: they share the host's caches and must not race. */
async function runAll(ctx: EntryContext, remaining: readonly EntryScenario[]): Promise<number> {
  const [scenario, ...rest] = remaining;
  if (scenario === undefined) return 0;
  const result = await scenario.run(ctx);
  const verdict = result.pass ? "pass" : "FAIL";
  process.stdout.write(`${scenario.id} ${verdict} ${JSON.stringify(result.observed)}\n`);
  return (result.pass ? 0 : 1) + (await runAll(ctx, rest));
}

async function main(ids: readonly string[]): Promise<number> {
  const scenarios = chosen(ids);
  const pin = readJson(contractPaths().pin, PinSchema);
  const host = await resolveHostBinary(pin);
  const cacheRoot = join(hostCacheDir(pin), "run-cache");
  mkdirSync(cacheRoot, { recursive: true });
  const work = mkdtempSync(join(tmpdir(), "toolu-entry-pack-"));
  try {
    const tarball = await packTarball(work);
    const failed = await runAll({ bin: host.bin, cacheRoot, tarball }, scenarios);
    process.stdout.write(
      `opencode-entry-smoke: ${scenarios.length - failed}/${scenarios.length} pass on opencode-ai@${host.version}\n`,
    );
    return failed === 0 ? 0 : 1;
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

if (import.meta.main) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (err: unknown) {
    if (!(err instanceof ContractError)) throw err;
    process.stderr.write(`opencode-entry-smoke: ${err.message}\n`);
    process.exitCode = 1;
  }
}
