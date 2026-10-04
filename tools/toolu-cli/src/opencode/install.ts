import { catalogNames, installOrder } from "../catalog/order";
import type { Marketplace } from "../catalog/types";
import type { InstallStep } from "../plugins/install";
import {
  PACKAGE,
  effectiveEntry,
  governingGlobal,
  pluginArray,
  tooluEntries,
  type PluginEntry,
} from "./entries";
import { appendJsonc, editJsonc, newConfigText, parseConfig, type ConfigFile } from "./jsonc";
import type { OpencodeScope } from "./paths";
import { closure, selectionText } from "./selection";
import {
  entriesIn,
  filesOf,
  installedScopes,
  type OpencodePlan,
  type OpencodeState,
  type PlannedWrite,
} from "./state";

interface InstallRequest {
  readonly names: readonly string[];
  readonly scope: OpencodeScope | undefined;
  readonly target: string;
}

function pick(files: readonly ConfigFile[], index: number): ConfigFile {
  const file = files[index];
  if (file === undefined) throw new Error(`missing config candidate ${index}`);
  return file;
}

/** The file a new entry goes to, so the host loads it rather than shadowing it. */
function entryFile(state: OpencodeState, scope: OpencodeScope): ConfigFile {
  const files = filesOf(state, scope);
  if (scope === "global") {
    return governingGlobal(files) ?? files.findLast((f) => f.text !== undefined) ?? pick(files, 1);
  }
  const jsonc = pick(files, 1);
  return (
    files.findLast((f) => pluginArray(f) !== undefined) ??
    (jsonc.text === undefined ? pick(files, 0) : jsonc)
  );
}

function withEntry(file: ConfigFile, spec: string): string {
  if (file.text === undefined) return newConfigText(spec);
  const array = pluginArray(file);
  if (array === undefined) return editJsonc(file.text, ["plugin"], [spec]);
  return appendJsonc(file.text, "plugin", spec);
}

function presentStep(entry: PluginEntry, target: string): InstallStep {
  const where = `${entry.spec} in ${entry.file}`;
  if (entry.spec === target) {
    return { name: PACKAGE, outcome: "already", detail: `already configured: ${where}`, argv: [] };
  }
  return {
    name: PACKAGE,
    outcome: "skew",
    detail: `configured as ${where}; this CLI installs ${target}; left untouched. Run \`npx @toolu/plugins update --host opencode\` to change it.`,
    argv: [],
  };
}

/** The state as it would read after `write`, to check what the host would load. */
function after(state: OpencodeState, write: PlannedWrite | undefined): OpencodeState {
  if (write === undefined) return state;
  const swap = (files: readonly ConfigFile[]): ConfigFile[] =>
    files.map((f) =>
      f.path === write.path ? { ...f, text: write.text, data: parseConfig(f.path, write.text) } : f,
    );
  return { ...state, global: swap(state.global), project: swap(state.project) };
}

/** A note when a project file's empty `plugin` list keeps the host from loading toolu. */
function shadowNote(state: OpencodeState): string {
  if (effectiveEntry(state.global, state.project) !== undefined) return "";
  const reset = state.project.findLast((f) => pluginArray(f)?.length === 0);
  return reset === undefined ? "" : `; no effect here: ${reset.path} sets an empty plugin list`;
}

/** Entries the host can load from `scope`; a lower-priority global array never loads (R1). */
function loadableEntries(state: OpencodeState, scope: OpencodeScope): readonly PluginEntry[] {
  if (scope === "project") return entriesIn(state, "project");
  const governing = governingGlobal(state.global);
  return governing === undefined ? [] : tooluEntries(governing);
}

/** With no scope, the entry the host loads (or any loadable one, when a reset shadows it). */
function existingEntry(state: OpencodeState, scope: OpencodeScope | undefined) {
  if (scope !== undefined) return loadableEntries(state, scope)[0];
  return (
    effectiveEntry(state.global, state.project) ??
    loadableEntries(state, "project")[0] ??
    loadableEntries(state, "global")[0]
  );
}

function packageStep(
  state: OpencodeState,
  request: InstallRequest,
): { step: InstallStep; write?: PlannedWrite } {
  const present = existingEntry(state, request.scope);
  if (present !== undefined) {
    const step = presentStep(present, request.target);
    const note = shadowNote(state);
    return { step: note === "" ? step : { ...step, outcome: "skew", detail: step.detail + note } };
  }
  const file = entryFile(state, request.scope ?? "global");
  const write = { path: file.path, text: withEntry(file, request.target) };
  const note = shadowNote(after(state, write));
  const step: InstallStep = {
    name: PACKAGE,
    outcome: note === "" ? "installed" : "skew",
    detail: `added ${request.target} to ${file.path}${note}`,
    argv: [],
    plan: `add ${request.target} to ${file.path}`,
  };
  return { step, write };
}

function pluginStep(
  name: string,
  added: boolean,
  path: string | undefined,
  state: OpencodeState,
): InstallStep {
  const off = state.disabled.has(name)
    ? ` (skills.${name} is false in ${state.paths.tooluConfig}, so it stays off)`
    : "";
  if (!added) return { name, outcome: "already", detail: `already enabled${off}`, argv: [] };
  if (path === undefined) {
    const detail = `enabled (no selection file, so every plugin is on)${off}`;
    return { name, outcome: "installed", detail, argv: [] };
  }
  return {
    name,
    outcome: "installed",
    detail: `enabled in ${path}${off}`,
    argv: [],
    plan: `enable ${name} in ${path}`,
  };
}

/** Selection changes for an install; see the spec's selection algorithm. */
function selectionPlan(
  state: OpencodeState,
  marketplace: Marketplace,
  request: InstallRequest,
  installedBefore: boolean,
): OpencodePlan<InstallStep> {
  const scope = request.scope ?? (state.selection.project === undefined ? "global" : "project");
  const all = catalogNames(marketplace);
  const want = request.names.length > 0 ? installOrder(marketplace, request.names) : all;
  const governing =
    scope === "global"
      ? state.selection.global
      : (state.selection.project ?? state.selection.global);
  const base = governing ?? (installedBefore ? all : []);
  const have = closure(marketplace, base);
  const added = want.filter((name) => !have.has(name));
  const next = [...new Set([...base, ...added])];
  const noFiles = state.selection.global === undefined && state.selection.project === undefined;
  const path = state.paths.selection[scope];
  const skip = added.length === 0 || (noFiles && all.every((name) => next.includes(name)));
  const writes = skip ? [] : [{ path, text: selectionText(next) }];
  const steps = want.map((name) =>
    pluginStep(name, added.includes(name), skip ? undefined : path, state),
  );
  return { steps, writes };
}

/** Ensures the package entry and the requested selection. */
export function opencodeInstall(
  state: OpencodeState,
  marketplace: Marketplace,
  request: InstallRequest,
): OpencodePlan<InstallStep> {
  const installedBefore = installedScopes(state).length > 0;
  const entry = packageStep(state, request);
  const selection = selectionPlan(state, marketplace, request, installedBefore);
  return {
    steps: [entry.step, ...selection.steps],
    writes: [...(entry.write === undefined ? [] : [entry.write]), ...selection.writes],
  };
}
