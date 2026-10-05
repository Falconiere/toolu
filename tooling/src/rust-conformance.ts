#!/usr/bin/env bun
/**
 * The Rust conformance leg (#409): `bun run test:rust-conformance [--manifest <file>]`
 * reads the ported hook entries from `fixtures/rust-ported.json`. An empty list
 * is a no-op. Otherwise it builds the release `toolu` binary, then runs
 * `test:unit` and `test:conformance` with `TOOLU_IMPL=rust:<entries>`, so each
 * ported entry faces the black-box suites its Bun bundle passes.
 * `CARGO` names the cargo executable, as cargo itself does.
 */
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { entryImplementation } from "@toolu/conformance/harness/entry-command";
import { z } from "zod";
import { envOr } from "./env.ts";

const ROOT = resolve(import.meta.dir, "../..");
export const DEFAULT_MANIFEST = resolve(ROOT, "fixtures/rust-ported.json");
export const SUITES = ["test:unit", "test:conformance"];

const Manifest = z.strictObject({ entries: z.array(z.string()) });

export class PortsError extends Error {}

/** The exact `TOOLU_IMPL` selector for `entries`, or null when nothing is ported. */
export function portSelector(entries: string[]): string | null {
  if (entries.length === 0) return null;
  const selector = `rust:${entries.join(",")}`;
  // The resolver's own grammar check: duplicates and malformed entries throw.
  try {
    entryImplementation("toolu", "probe", { TOOLU_IMPL: selector });
  } catch (error) {
    throw new PortsError(error instanceof Error ? error.message : String(error), { cause: error });
  }
  return selector;
}

/** The ported entries listed in `file`; an unreadable or malformed file throws. */
export function readPorted(file: string): string[] {
  let doc: unknown;
  try {
    doc = JSON.parse(readFileSync(file, "utf8"));
  } catch (error: unknown) {
    throw new PortsError(`${file} is not readable JSON`, { cause: error });
  }
  const parsed = Manifest.safeParse(doc);
  if (!parsed.success) throw new PortsError(`${file} must be { "entries": [<plugin>/<entry>...] }`);
  return parsed.data.entries;
}

function step(argv: string[], env: NodeJS.ProcessEnv): number {
  const [command = "", ...args] = argv;
  const result = spawnSync(command, args, { cwd: ROOT, env, stdio: "inherit" });
  if (result.error !== undefined) {
    process.stderr.write(`rust-conformance: ${command}: ${result.error.message}\n`);
    return 1;
  }
  return result.status ?? 1;
}

function manifestArg(argv: string[]): string {
  if (argv.length === 0) return DEFAULT_MANIFEST;
  if (argv.length === 2 && argv[0] === "--manifest" && argv[1] !== undefined) {
    return resolve(argv[1]);
  }
  throw new PortsError("usage: rust-conformance.ts [--manifest <file>]");
}

function main(argv: string[]): number {
  let selector: string | null;
  try {
    selector = portSelector(readPorted(manifestArg(argv)));
  } catch (error) {
    if (!(error instanceof PortsError)) throw error;
    process.stderr.write(`rust-conformance: ${error.message}\n`);
    return 2;
  }
  if (selector === null) {
    process.stdout.write("rust-conformance: no ported entries; nothing to run\n");
    return 0;
  }
  process.stdout.write(`rust-conformance: TOOLU_IMPL=${selector}\n`);
  const cargo = envOr("CARGO", "cargo");
  if (step([cargo, "build", "--release", "--locked", "--bin", "toolu"], process.env) !== 0) {
    process.stderr.write("rust-conformance: cargo build of the toolu binary failed\n");
    return 1;
  }
  const env = { ...process.env, TOOLU_IMPL: selector };
  const failed = SUITES.filter((suite) => step([process.execPath, "run", suite], env) !== 0);
  if (failed.length > 0) {
    process.stderr.write(`rust-conformance: ${failed.join(", ")} failed under ${selector}\n`);
    return 1;
  }
  return 0;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
