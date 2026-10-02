/**
 * Contract file locations and live-result comparison (#335). The live CLI
 * builds a `ProbeResults` from a run; without `--write` it must match the
 * committed file probe for probe, so a host change cannot pass silently.
 */
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { z } from "zod";
import { envOr } from "../env.ts";
import type { ProbeResult, ProbeResults } from "./schema.ts";

export const ROOT = resolve(import.meta.dir, "../../..");

type ContractPaths = { dir: string; pin: string; results: string; matrix: string };

export function contractPaths(
  env: Record<string, string | undefined> = process.env,
): ContractPaths {
  const dir = envOr(
    "TOOLU_OPENCODE_CONTRACT_DIR",
    join(ROOT, "tools/toolu-opencode/contract"),
    env,
  );
  return {
    dir,
    pin: join(dir, "pin.json"),
    results: join(dir, "probe-results.json"),
    matrix: join(dir, "capability-matrix.json"),
  };
}

const SdkManifest = z.looseObject({ version: z.string() });

/** The `@opencode-ai/plugin` version the host installed into a project's `.opencode/`. */
export function provisionedSdkVersion(project: string): string | null {
  const manifest = join(project, ".opencode/node_modules/@opencode-ai/plugin/package.json");
  if (!existsSync(manifest)) return null;
  const parsed = SdkManifest.safeParse(JSON.parse(readFileSync(manifest, "utf8")));
  return parsed.success ? parsed.data.version : null;
}

function same(a: ProbeResult, b: ProbeResult): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** One line per difference between committed and live results; empty when they agree. */
export function resultDrift(committed: ProbeResults, live: ProbeResults): string[] {
  const drift: string[] = [];
  for (const key of ["cliVersion", "provisionedSdkVersion"] as const) {
    if (committed.host[key] !== live.host[key])
      drift.push(`host.${key}: ${committed.host[key]} -> ${live.host[key]}`);
  }
  const byId = new Map(committed.probes.map((p) => [p.id, p]));
  for (const probe of live.probes) {
    const before = byId.get(probe.id);
    if (before === undefined) drift.push(`${probe.id}: missing from committed results`);
    else if (!same(before, probe)) {
      drift.push(
        `${probe.id}: ${before.verdict} ${JSON.stringify(before.observed)} -> ${probe.verdict} ${JSON.stringify(probe.observed)}`,
      );
    }
  }
  if (committed.probes.length !== live.probes.length)
    drift.push(`probe count: ${committed.probes.length} -> ${live.probes.length}`);
  return drift;
}
