import { readFile } from "node:fs/promises";
import { UsageError } from "../exit";
import { SELECTION_VERSION, selectionSchema, versionProbeSchema, type Selection } from "./schema";

function majorOf(version: string): number {
  const major = Number.parseInt(version.split(".")[0] ?? "", 10);
  return Number.isNaN(major) ? 0 : major;
}

function assertVersion(parsed: unknown, path: string): void {
  const probe = versionProbeSchema.safeParse(parsed);
  if (!probe.success) {
    throw new UsageError(`${path} has no numeric "version" field`);
  }
  if (probe.data.version !== SELECTION_VERSION) {
    throw new UsageError(
      `${path} declares version ${probe.data.version}; this CLI understands version ${SELECTION_VERSION}`,
    );
  }
}

/**
 * Reads a selection file, rejecting an unknown format version or a file written
 * by a newer major of the CLI rather than guessing at its meaning.
 */
export async function readSelection(path: string, cliVersion: string): Promise<Selection> {
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch {
    throw new UsageError(`cannot read selection file: ${path}`);
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new UsageError(`${path} is not valid JSON: ${detail}`);
  }
  assertVersion(parsed, path);
  const result = selectionSchema.safeParse(parsed);
  if (!result.success) {
    throw new UsageError(`${path} is malformed: ${result.error.message}`);
  }
  const written = result.data.generatorVersion;
  if (written !== undefined && majorOf(written) > majorOf(cliVersion)) {
    throw new UsageError(
      `${path} was written by toolu ${written}; this CLI is ${cliVersion}. Upgrade the CLI.`,
    );
  }
  return result.data;
}
