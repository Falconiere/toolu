import { catalogNames } from "../catalog/order";
import type { Marketplace } from "../catalog/types";
import type { CatalogState } from "../plugins/list";
import { PACKAGE, effectiveEntry, versionOf } from "./entries";
import { effectiveEnabled } from "./selection";
import type { OpencodeState } from "./state";

/** The catalog as OpenCode would load it now, plus a one-line source summary. */
export function opencodeList(
  state: OpencodeState,
  marketplace: Marketplace,
): { readonly entries: readonly CatalogState[]; readonly header: string } {
  const entry = effectiveEntry(state.global, state.project);
  if (entry === undefined) {
    return {
      entries: catalogNames(marketplace).map((name) => ({
        name,
        installed: false,
        version: undefined,
        enabled: false,
      })),
      header: `${PACKAGE}: not configured\n`,
    };
  }
  const source = state.selection.project === undefined ? "global" : "project";
  const listed = state.selection[source];
  const enabled = effectiveEnabled(marketplace, listed, state.disabled);
  const selection = listed === undefined ? "none (every plugin)" : state.paths.selection[source];
  return {
    entries: catalogNames(marketplace).map((name) => ({
      name,
      installed: true,
      version: versionOf(entry.spec),
      enabled: enabled.has(name),
    })),
    header: `${PACKAGE}: ${entry.spec} in ${entry.file}; selection: ${selection}\n`,
  };
}
