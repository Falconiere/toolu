import { isCancel, multiselect, select } from "@clack/prompts";
import type { Host } from "../args/types";
import type { CatalogEntry } from "../catalog/types";
import { CliError, EXIT } from "../exit";

/** Maps a Clack cancel sentinel to exit 130; otherwise returns the value. */
export function rejectIfCancelled<T>(value: T | symbol): T {
  if (isCancel(value)) throw new CliError(EXIT.cancelled, "cancelled");
  return value;
}

/** Multi-select among available hosts (install). */
export async function selectHosts(candidates: readonly Host[]): Promise<readonly Host[]> {
  const value = await multiselect({
    message: "Which hosts should receive the plugins?",
    options: candidates.map((host) => ({ value: host, label: host })),
    initialValues: [...candidates],
    required: true,
  });
  return rejectIfCancelled(value);
}

/** Single-select among available hosts (list / remove / update). */
export async function selectHost(candidates: readonly Host[]): Promise<Host> {
  const value = await select({
    message: "Which host?",
    options: candidates.map((host) => ({ value: host, label: host })),
  });
  return rejectIfCancelled(value);
}

/** Multi-select catalog plugins; defaults to every entry selected. */
export async function selectPlugins(
  plugins: readonly CatalogEntry[],
): Promise<readonly string[]> {
  const value = await multiselect({
    message: "Which plugins should be installed?",
    options: plugins.map((plugin) => ({
      value: plugin.name,
      label: plugin.name,
      hint: plugin.description,
    })),
    initialValues: plugins.map((plugin) => plugin.name),
    required: true,
  });
  return rejectIfCancelled(value);
}

export type SelectHosts = (candidates: readonly Host[]) => Promise<readonly Host[]>;
export type SelectHost = (candidates: readonly Host[]) => Promise<Host>;
export type SelectPlugins = (plugins: readonly CatalogEntry[]) => Promise<readonly string[]>;
