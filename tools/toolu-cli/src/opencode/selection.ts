import { readFile } from "node:fs/promises";
import { z } from "zod";
import { catalogNames, installOrder } from "../catalog/order";
import type { Marketplace } from "../catalog/types";
import { CliError, EXIT } from "../exit";

/** The adapter's selection schema (`tools/toolu-opencode/src/inventory/selection.ts`). */
const selectionFileSchema = z
  .object({ version: z.literal(1), enabled: z.array(z.string().min(1)) })
  .strict();

const skillsSchema = z.looseObject({ skills: z.record(z.string(), z.boolean()).optional() });

function isMissing(error: unknown): boolean {
  return error instanceof Error && "code" in error && error.code === "ENOENT";
}

/** The enabled list of a selection file, undefined when absent; invalid files fail closed. */
export async function readSelectionFile(path: string): Promise<readonly string[] | undefined> {
  let text: string;
  try {
    text = await readFile(path, "utf8");
  } catch (error) {
    if (isMissing(error)) return undefined;
    throw new CliError(EXIT.failed, `cannot read ${path}: ${String(error)}`);
  }
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch (error) {
    throw new CliError(EXIT.failed, `invalid ${path}: ${String(error)}; nothing was written`);
  }
  const parsed = selectionFileSchema.safeParse(raw);
  if (!parsed.success) {
    throw new CliError(
      EXIT.failed,
      `invalid ${path}: ${z.prettifyError(parsed.error)}; nothing was written`,
    );
  }
  return parsed.data.enabled;
}

export function selectionText(enabled: readonly string[]): string {
  return `${JSON.stringify({ version: 1, enabled }, null, 2)}\n`;
}

/**
 * Plugins turned off by `skills.<name>: false` in the project's toolu config.
 * Like the adapter, an unreadable or malformed config disables nothing.
 */
export async function skillsDisabled(tooluConfigPath: string): Promise<ReadonlySet<string>> {
  try {
    const parsed = skillsSchema.safeParse(JSON.parse(await readFile(tooluConfigPath, "utf8")));
    if (!parsed.success) return new Set();
    const skills = parsed.data.skills ?? {};
    return new Set(Object.keys(skills).filter((name) => skills[name] === false));
  } catch {
    return new Set();
  }
}

/** The names plus every catalog plugin they depend on; names outside the catalog are dropped. */
export function closure(marketplace: Marketplace, names: readonly string[]): ReadonlySet<string> {
  const known = new Set(catalogNames(marketplace));
  const seeds = names.filter((name) => known.has(name));
  return new Set(seeds.length === 0 ? [] : installOrder(marketplace, seeds));
}

/** What the adapter enables: listed catalog names not disabled by skills, then closed. */
export function effectiveEnabled(
  marketplace: Marketplace,
  listed: readonly string[] | undefined,
  disabled: ReadonlySet<string>,
): ReadonlySet<string> {
  const names = listed ?? catalogNames(marketplace);
  return closure(
    marketplace,
    names.filter((name) => !disabled.has(name)),
  );
}

/** Enabled plugins outside `removing` that depend, directly or not, on `name`. */
export function dependentsBlocking(
  marketplace: Marketplace,
  enabled: ReadonlySet<string>,
  removing: ReadonlySet<string>,
  name: string,
): readonly string[] {
  return [...enabled].filter(
    (other) => other !== name && !removing.has(other) && closure(marketplace, [other]).has(name),
  );
}
