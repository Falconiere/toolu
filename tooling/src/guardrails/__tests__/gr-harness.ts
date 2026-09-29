/**
 * Test harness for the ported upstream guardrails suite: run the real
 * TypeScript runner inside a real fixture repo and read what it printed.
 */
import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import type { EnvPatch } from "@toolu/conformance/harness/spawn";
import { z } from "zod";

export { FAKE_AWS_KEY } from "./fixture-tree.ts";

export const RUN_TS = resolve(import.meta.dir, "../run.ts");

export type GrResult = { exit: number; out: string };

const CLEAN_ENV: EnvPatch = {
  GR_CONFIG: undefined,
  GR_WS_CHILD: undefined,
  GR_PATH_PREFIX: undefined,
};

/** `cd root && bun run.ts args` with stdout and stderr combined, as `2>&1`. */
export async function gr(
  root: string,
  args: readonly string[],
  opts: { stdin?: string; env?: EnvPatch; runner?: string } = {},
): Promise<GrResult> {
  const res = await run([process.execPath, opts.runner ?? RUN_TS, ...args], {
    cwd: root,
    stdin: opts.stdin ?? "",
    env: { ...CLEAN_ENV, ...opts.env },
    timeoutMs: 60_000,
  });
  return { exit: res.exitCode, out: res.stderr + res.stdout };
}

/** Violations one check reported. */
export function count(out: string, check: string): number {
  return out.split("\n").filter((line) => line.startsWith(`guardrails[${check}]`)).length;
}

const Json = z.record(z.string(), z.unknown());

/** Rewrite a JSON file in place (the upstream suite's `jq … > t && mv t`). */
export function editJson(path: string, edit: (doc: Record<string, unknown>) => void): void {
  const doc = Json.parse(JSON.parse(readFileSync(path, "utf8")));
  edit(doc);
  writeFileSync(path, `${JSON.stringify(doc, null, 2)}\n`);
}

export function editConfig(root: string, edit: (doc: Record<string, unknown>) => void): void {
  editJson(join(root, "guardrails.config.json"), edit);
}

/** `{ ...doc[key], ...patch }` for a nested config object. */
export function merge(
  doc: Record<string, unknown>,
  key: string,
  patch: Record<string, unknown>,
): void {
  doc[key] = { ...Json.parse(doc[key] ?? {}), ...patch };
}
