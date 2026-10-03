/** The shared `main` of the pinned-host smoke entries: run every scenario, or the one named. */
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { hostCacheDir, resolveHostBinary } from "./install.ts";
import { runPretoolScenarios, type PretoolScenario } from "./pretool-shared.ts";
import { contractPaths } from "./results.ts";
import { ContractError, PinSchema, readJson } from "./schema.ts";

/** Exit code 0 when every selected scenario passes; `name` labels the summary line. */
export async function runSmoke(
  name: string,
  scenarios: readonly PretoolScenario[],
  only: string | undefined,
): Promise<number> {
  const pin = readJson(contractPaths().pin, PinSchema);
  const host = await resolveHostBinary(pin, process.env, 90_000);
  const cacheRoot = join(hostCacheDir(pin), "run-cache");
  mkdirSync(cacheRoot, { recursive: true });
  const selected =
    only === undefined ? scenarios : scenarios.filter((scenario) => scenario.id === only);
  if (selected.length === 0) throw new ContractError(`unknown ${name} scenario: ${only}`);
  const failed = await runPretoolScenarios({ bin: host.bin, cacheRoot }, selected);
  process.stdout.write(
    `${name}: ${selected.length - failed}/${selected.length} pass on opencode-ai@${host.version}\n`,
  );
  return failed === 0 ? 0 : 1;
}
