// Change-gated poll driver for the dashboard. Each tick computes a cheap
// fingerprint (discovered ledger mtimes + selection + the rendered project's
// activity stat) and only rebuilds MultiDashboardState when it changed. The
// target is resolved exactly as buildMultiState resolves it — explicit
// selection, else the most-recently-active project (buildSummaries[0]) — so the
// gate fingerprints the project that will actually be rendered, not a stale idle
// one. An idle tick reads ledgers and stats but never parses a transcript. The
// server drives tick() on an interval; tests drive it directly for determinism.

import type { DashboardConfig } from "./config.ts";
import { discoverProjects, type DiscoveredProject } from "./discovery.ts";
import {
  buildMultiStateFrom,
  buildSummaries,
  defaultSelectedId,
  type MultiDashboardState,
} from "./aggregate.ts";
import type { LiveActivitySource } from "./activity/source.ts";

/** A poll driver: change the selection, then tick to (maybe) get fresh state. */
export interface Watcher {
  /** Current selected project id (null = default to most-recently-active). */
  selected(): string | null;
  /** Change the selection; the next tick will rebuild for it. */
  setSelected(id: string | null): void;
  /** Recompute the fingerprint; return fresh state if anything changed, else null. */
  tick(nowMs: number): MultiDashboardState | null;
  /** The last state produced by a tick (null before the first change). */
  current(): MultiDashboardState | null;
}

/** Create a watcher over `cfg`/`src`. Stateless aside from the last fingerprint + state. */
export function createWatcher(cfg: DashboardConfig, src: LiveActivitySource): Watcher {
  let selectedId: string | null = null;
  let lastFp: string | null = null; // null until the first tick, so the first tick always emits
  let last: MultiDashboardState | null = null;

  // The target is resolved by the caller (tick) from the same scan used to build
  // the state, so the gate fingerprints exactly the project that will be rendered.
  // ledgerSig still spans all projects, so a newly-active one still triggers a tick.
  function fingerprint(projects: DiscoveredProject[], target: string): string {
    const ledgerSig = projects
      .map((p) => `${p.id}:${p.ledgerMtimeMs}`)
      .sort()
      .join("|");
    const sel = projects.find((p) => p.id === target);
    const actSig = sel && src.activityFingerprint ? src.activityFingerprint(sel) : 0;
    return `${ledgerSig}#sel:${selectedId ?? ""}#tgt:${target}#act:${actSig}`;
  }

  return {
    selected: () => selectedId,
    setSelected(id: string | null): void {
      selectedId = id;
    },
    tick(nowMs: number): MultiDashboardState | null {
      // One scan per tick, shared by the change-gate and the state build, so the
      // fingerprinted target always matches the rendered selection and a single
      // Date.now() decides activeness (no boundary skew between the two).
      const discovered = discoverProjects(cfg);
      const summaries = buildSummaries(cfg, src, discovered);
      const target = defaultSelectedId(summaries, selectedId);
      const fp = fingerprint(discovered, target ?? "");
      if (fp === lastFp) return null;
      lastFp = fp;
      last = buildMultiStateFrom(cfg, src, summaries, selectedId, nowMs);
      return last;
    },
    current: () => last,
  };
}
