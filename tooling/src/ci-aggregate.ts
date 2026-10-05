#!/usr/bin/env bun
/**
 * A required aggregate (#458): `bun run tooling/src/ci-aggregate.ts <workflow>`
 * judges `NEEDS` (`${{ toJSON(needs) }}`) against the workflow's gated jobs in
 * `.github/ci-paths.json`. Exits 0 when the aggregate passes, 1 when it fails,
 * and names every failing job. `CI_CHANGES_ROOT` points it at another checkout.
 */
import { resolve } from "node:path";
import { judge, NeedsSchema } from "./ci-paths/aggregate.ts";
import { type CiPaths, CiPathsError, loadRepoCiPaths } from "./ci-paths/config.ts";
import { envOr } from "./env.ts";

function fail(message: string): number {
  process.stderr.write(`ci-aggregate: ${message}\n`);
  return 1;
}

function main(argv: string[]): number {
  const name = argv[0];
  if (name === undefined) return fail("usage: ci-aggregate.ts <workflow file name>");
  let workflows: CiPaths["workflows"];
  try {
    workflows = loadRepoCiPaths(resolve(envOr("CI_CHANGES_ROOT", process.cwd()))).workflows;
  } catch (error) {
    if (!(error instanceof CiPathsError)) throw error;
    return fail(error.message);
  }
  const workflow = workflows[name];
  if (workflow === undefined) return fail(`${name} has no entry in the data file`);
  let raw: unknown;
  try {
    raw = JSON.parse(envOr("NEEDS", ""));
  } catch {
    return fail("NEEDS is not JSON");
  }
  const needs = NeedsSchema.safeParse(raw);
  if (!needs.success) return fail("NEEDS is not the toJSON(needs) shape");
  const verdict = judge(workflow, needs.data);
  const stream = verdict.ok ? process.stdout : process.stderr;
  stream.write(`${verdict.lines.join("\n")}\n`);
  return verdict.ok ? 0 : 1;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
