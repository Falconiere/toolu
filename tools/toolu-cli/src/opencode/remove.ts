import { catalogNames, installOrder } from "../catalog/order";
import type { Marketplace } from "../catalog/types";
import { UsageError } from "../exit";
import type { RemoveStep } from "../plugins/remove";
import { PACKAGE, pluginArray, tooluEntries } from "./entries";
import { editJsonc, removeJsonc, type ConfigFile } from "./jsonc";
import type { OpencodeScope } from "./paths";
import { closure, dependentsBlocking, selectionText } from "./selection";
import { filesOf, installedScopes, type OpencodePlan, type OpencodeState } from "./state";

const CORE = "toolu";

function done(name: string, detail: string, plan?: string): RemoveStep {
  return { name, removed: true, detail, argv: [], ...(plan === undefined ? {} : { plan }) };
}

/** The scopes a package-level verb acts on; ambiguity is a usage error, not a guess. */
export function packageScopes(
  state: OpencodeState,
  scope: OpencodeScope | undefined,
): readonly OpencodeScope[] {
  if (scope !== undefined) return [scope];
  const scopes = installedScopes(state);
  if (scopes.length > 1) {
    throw new UsageError(
      `${PACKAGE} is configured in both global and project OpenCode config; choose one with --scope user or --scope project`,
    );
  }
  return scopes;
}

/** Drops every toolu entry; an emptied array loses its key, since `[]` resets the host's list. */
function withoutEntries(file: ConfigFile): string {
  const text = file.text ?? "";
  const entries = tooluEntries(file);
  if (entries.length === (pluginArray(file)?.length ?? 0))
    return editJsonc(text, ["plugin"], undefined);
  return entries
    .toReversed()
    .reduce((current, entry) => removeJsonc(current, "plugin", entry.index), text);
}

/** A lower-priority global file whose list applies again once `file` loses its key. */
function resurfaced(state: OpencodeState, file: ConfigFile): string {
  const index = state.global.findIndex((f) => f.path === file.path);
  if (index <= 0 || tooluEntries(file).length !== pluginArray(file)?.length) return "";
  const lower = state.global.slice(0, index).findLast((f) => pluginArray(f) !== undefined);
  return lower === undefined ? "" : `; ${lower.path}'s plugin list applies again`;
}

function removePackage(
  state: OpencodeState,
  names: readonly string[],
  scope: OpencodeScope | undefined,
): OpencodePlan<RemoveStep> {
  const files = packageScopes(state, scope)
    .flatMap((s) => filesOf(state, s))
    .filter((file) => tooluEntries(file).length > 0);
  const others = names
    .filter((name) => name !== CORE)
    .map((name) => done(name, `removed with ${PACKAGE}`));
  if (files.length === 0) {
    return {
      steps: [done(CORE, `${PACKAGE} is not configured; nothing to remove`), ...others],
      writes: [],
    };
  }
  const kept = (["project", "global"] as const)
    .filter((s) => state.selection[s] !== undefined)
    .map((s) => `; selection kept at ${state.paths.selection[s]}`)
    .join("");
  const where = files.map((file) => file.path).join(", ");
  const notes = files.map((file) => resurfaced(state, file)).join("");
  const step = done(
    CORE,
    `removed ${PACKAGE} from ${where}${kept}${notes}`,
    `remove ${PACKAGE} from ${where}`,
  );
  const writes = files.map((file) => ({ path: file.path, text: withoutEntries(file) }));
  return { steps: [step, ...others], writes };
}

/** Names that can go: enabled, and not needed by an enabled plugin that stays. */
function removable(
  marketplace: Marketplace,
  enabled: ReadonlySet<string>,
  names: readonly string[],
): ReadonlySet<string> {
  let removing = new Set(names.filter((name) => enabled.has(name)));
  for (;;) {
    const current = removing;
    const kept = [...current].filter(
      (name) => dependentsBlocking(marketplace, enabled, current, name).length === 0,
    );
    if (kept.length === current.size) return current;
    removing = new Set(kept);
  }
}

function removeLeaves(
  state: OpencodeState,
  marketplace: Marketplace,
  names: readonly string[],
  scope: OpencodeScope | undefined,
): OpencodePlan<RemoveStep> {
  if (installedScopes(state).length === 0) {
    return {
      steps: names.map((name) => done(name, `${PACKAGE} is not configured; nothing to remove`)),
      writes: [],
    };
  }
  const target = scope ?? (state.selection.project === undefined ? "global" : "project");
  const governing =
    target === "global"
      ? state.selection.global
      : (state.selection.project ?? state.selection.global);
  const base = governing ?? catalogNames(marketplace);
  const enabled = closure(marketplace, base);
  const removing = removable(marketplace, enabled, names);
  const path = state.paths.selection[target];
  const steps = names.map((name): RemoveStep => {
    if (!enabled.has(name)) return done(name, "not enabled; nothing to change");
    if (removing.has(name)) return done(name, `disabled in ${path}`, `disable ${name} in ${path}`);
    const blockers = dependentsBlocking(marketplace, enabled, removing, name);
    return {
      name,
      removed: false,
      detail: `required by ${blockers.join(", ")}; remove them too`,
      argv: [],
    };
  });
  const next = base.filter((name) => !removing.has(name));
  const writes = removing.size === 0 ? [] : [{ path, text: selectionText(next) }];
  return { steps, writes };
}

/** `remove toolu` drops the package; other names leave the selection. */
export function opencodeRemove(
  state: OpencodeState,
  marketplace: Marketplace,
  names: readonly string[],
  scope: OpencodeScope | undefined,
): OpencodePlan<RemoveStep> {
  installOrder(marketplace, names);
  if (names.includes(CORE)) return removePackage(state, names, scope);
  return removeLeaves(state, marketplace, names, scope);
}
