import { readConfigFile, type ConfigFile } from "./jsonc";
import { opencodePaths, projectRootOf, type OpencodePaths, type OpencodeScope } from "./paths";
import { readSelectionFile, skillsDisabled } from "./selection";
import { tooluEntries, type PluginEntry } from "./entries";

/** A whole-file write a verb plans; dry runs print it instead. */
export interface PlannedWrite {
  readonly path: string;
  readonly text: string;
}

/** What a verb decided: the steps to report and the files to write. */
export interface OpencodePlan<Step> {
  readonly steps: readonly Step[];
  readonly writes: readonly PlannedWrite[];
}

/** Everything the OpenCode verbs decide from, read once per run. */
export interface OpencodeState {
  readonly paths: OpencodePaths;
  readonly global: readonly ConfigFile[];
  readonly project: readonly ConfigFile[];
  readonly selection: Readonly<Record<OpencodeScope, readonly string[] | undefined>>;
  readonly disabled: ReadonlySet<string>;
}

export async function loadState(cwd: string, env: NodeJS.ProcessEnv): Promise<OpencodeState> {
  const paths = opencodePaths(await projectRootOf(cwd, env), env);
  const global = await Promise.all(paths.globalFiles.map(readConfigFile));
  const project = await Promise.all(paths.projectFiles.map(readConfigFile));
  return {
    paths,
    global,
    project,
    selection: {
      global: await readSelectionFile(paths.selection.global),
      project: await readSelectionFile(paths.selection.project),
    },
    disabled: await skillsDisabled(paths.tooluConfig),
  };
}

export function filesOf(state: OpencodeState, scope: OpencodeScope): readonly ConfigFile[] {
  return scope === "global" ? state.global : state.project;
}

/** Toolu entries per scope. */
export function entriesIn(state: OpencodeState, scope: OpencodeScope): readonly PluginEntry[] {
  return filesOf(state, scope).flatMap(tooluEntries);
}

/** A note for a global selection edit that a project selection overrides here. */
export function overriddenNote(state: OpencodeState, scope: OpencodeScope): string {
  if (scope !== "global" || state.selection.project === undefined) return "";
  return `; no effect in this project: ${state.paths.selection.project} governs it`;
}

/** The scopes holding a toolu entry. */
export function installedScopes(state: OpencodeState): readonly OpencodeScope[] {
  return (["global", "project"] as const).filter((scope) => entriesIn(state, scope).length > 0);
}
