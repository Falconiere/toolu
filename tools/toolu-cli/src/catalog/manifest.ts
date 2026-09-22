import { readFile } from "node:fs/promises";
import { CliError, EXIT, UsageError } from "../exit";
import { marketplaceSchema, type Marketplace } from "./types";

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
