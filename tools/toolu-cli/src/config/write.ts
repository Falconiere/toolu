import { mkdir, rename, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { SELECTION_VERSION, type Selection } from "./schema";

/** Default location of the selection file, beside the tracked .toolu/skills convention. */
export function selectionPath(projectRoot: string): string {
  return join(projectRoot, ".toolu", "plugins.json");
}

/**
 * Writes the selection atomically: a temporary file in the same directory, then a
 * rename. A concurrent reader sees either the old file or the new one, never a
 * partially written one.
 */
export async function writeSelection(path: string, selection: Selection): Promise<void> {
  const directory = dirname(path);
  await mkdir(directory, { recursive: true });
  const body = `${JSON.stringify({ ...selection, version: SELECTION_VERSION }, null, 2)}\n`;
  const temporary = join(directory, `.plugins.json.${process.pid}.${Date.now()}`);
  await writeFile(temporary, body, "utf8");
  await rename(temporary, path);
}
