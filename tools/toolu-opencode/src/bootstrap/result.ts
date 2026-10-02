/** Bootstrap Ready / NotReady union (#211), with what each plugin's startup contributed (#342). */

/** One startup entry that ran, with the context it produced for lifecycle delivery (OP-07). */
export type EntryOutcome = { entry: string; additionalContext?: string; systemMessage?: string };

/** One selected plugin whose startup completed, in startup order. */
export type PluginStartup = { plugin: string; entries: EntryOutcome[]; artifacts: string[] };

export type ReadyResult = {
  status: "ready";
  /** Every verified registry module and helper this run produced. */
  artifacts: string[];
  plugins: PluginStartup[];
  /** Non-fatal notes: kept user files, removed contributions, entry stderr. */
  diagnostics: string[];
};

export type NotReadyResult = {
  status: "not-ready";
  reason: string;
};

export type BootstrapResult = ReadyResult | NotReadyResult;

export function notReady(reason: string): NotReadyResult {
  return { status: "not-ready", reason };
}
