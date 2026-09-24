import { readFile } from "node:fs/promises";
import { basename, resolve } from "node:path";
import { CliError, EXIT, UsageError } from "../exit";
import { marketplaceSchema, type Marketplace } from "./types";

/**
 * Where the manifest lives, given the directory of the running CLI.
 *
 * The published bundle (dist/cli.js) reads the copy its build put in the
 * sibling assets/ folder, because there is no repository beside an installed
 * tarball. Run from source, the CLI reads the repository catalog three levels
 * up, and never an assets/ copy that a build from before the npm/ publish
 * folder left beside src/.
 */
export function manifestCandidates(here: string): readonly string[] {
  if (basename(here) === "src") return [resolve(here, "../../../.claude-plugin/marketplace.json")];
  return [resolve(here, "../assets/marketplace.json")];
}

/** Reads and validates a marketplace manifest from disk. */
export async function readMarketplace(path: string): Promise<Marketplace> {
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch {
    throw new CliError(EXIT.failed, `cannot read marketplace manifest: ${path}`);
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new UsageError(`marketplace manifest is not valid JSON: ${path}: ${detail}`);
  }
  const result = marketplaceSchema.safeParse(parsed);
  if (!result.success) {
    throw new UsageError(`marketplace manifest is malformed: ${path}: ${result.error.message}`);
  }
  return result.data;
}
