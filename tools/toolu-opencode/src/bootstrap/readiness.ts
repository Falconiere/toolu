/**
 * The startup verdict (#342). Ready means every selected plugin's startup ran
 * and verified in this run, and every contribution of an unselected plugin was
 * taken back. Nothing already on disk can make startup ready: no marker, no
 * other plugin's module, no earlier session's file. The ownership ledger is
 * settled here too, whatever the verdict, so a later startup can clean up.
 */
import type { PluginManifest } from "../inventory/types.ts";
import { retireHelpers, writeLedger, type Cleanup, type Ledger } from "./ledger.ts";
import type { OwnedHelper, Verified } from "./records.ts";
import { notReady, type BootstrapResult, type EntryOutcome } from "./result.ts";

export type PluginRun =
  | { status: "ready"; plugin: PluginManifest; verified: Verified; entries: EntryOutcome[] }
  | { status: "failed"; plugin: PluginManifest; verified: Verified; failure: string };

export type Settlement = {
  dataRoot: string;
  runs: readonly PluginRun[];
  /** The ledger this startup began with. */
  previous: Ledger;
  /** The ledger being built: entries kept for unselected plugins, then one per run. */
  next: Ledger;
  cleanup: Cleanup;
};

/** The reason a deny-all carries stays readable in a host log. */
const MAX_REASON_CHARS = 4_000;

function bounded(reason: string): string {
  return reason.length > MAX_REASON_CHARS ? `${reason.slice(0, MAX_REASON_CHARS)}…` : reason;
}

function unique(helpers: readonly OwnedHelper[]): OwnedHelper[] {
  const byPath = new Map(helpers.map((helper) => [helper.path, helper]));
  return [...byPath.values()];
}

/**
 * Record what each plugin now owns. A ready plugin retires the helpers it no
 * longer publishes; a failed one keeps every helper it ever published, so a
 * later startup can still take them back.
 */
function settleLedger(settlement: Settlement): void {
  const { runs, previous, next, cleanup } = settlement;
  for (const run of runs) {
    const { name, spec } = run.plugin;
    const old = previous.plugins[name]?.helpers ?? [];
    const current = run.verified.helpers;
    let owned: OwnedHelper[];
    if (run.status === "ready") {
      const stale = old.filter((helper) => !current.some((c) => c.path === helper.path));
      owned = unique([...current, ...retireHelpers(name, stale, settlement.dataRoot, cleanup)]);
    } else {
      owned = unique([...old, ...current]);
    }
    if (owned.length > 0) next.plugins[name] = { spec, helpers: owned };
  }
  const failure = writeLedger(settlement.dataRoot, next);
  if (failure !== undefined) cleanup.failures.push(failure);
}

export function settle(settlement: Settlement): BootstrapResult {
  settleLedger(settlement);
  const { runs, cleanup } = settlement;
  const failures = [
    ...runs.flatMap((run) => (run.status === "failed" ? [run.failure] : [])),
    ...cleanup.failures,
  ];
  if (failures.length > 0) return notReady(bounded(failures.join("; ")));
  const plugins = runs.flatMap((run) =>
    run.status === "ready"
      ? [{ plugin: run.plugin.name, entries: run.entries, artifacts: run.verified.artifacts }]
      : [],
  );
  return {
    status: "ready",
    artifacts: plugins.flatMap((plugin) => plugin.artifacts),
    plugins,
    diagnostics: [...cleanup.diagnostics, ...runs.flatMap((run) => run.verified.diagnostics)],
  };
}
