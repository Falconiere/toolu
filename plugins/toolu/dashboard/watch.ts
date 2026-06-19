// Change-gated poll driver for the dashboard. Each tick computes a cheap
// fingerprint (discovered ledger mtimes + selection + the selected project's
// activity stat) and only rebuilds MultiDashboardState when it changed — so an
// idle tick costs a walk + a few stats, never a transcript parse. The server
// drives tick() on an interval; tests drive it directly for determinism.

import type { DashboardConfig } from "./config.ts";
import { discoverProjects } from "./discovery.ts";
import { buildMultiState, type MultiDashboardState } from "./aggregate.ts";
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

  function fingerprint(): string {
    const projects = discoverProjects(cfg);
    const ledgerSig = projects
      .map((p) => `${p.id}:${p.ledgerMtimeMs}`)
      .sort()
      .join("|");
    const target = selectedId ?? projects.sort((a, b) => b.ledgerMtimeMs - a.ledgerMtimeMs)[0]?.id ?? "";
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
      const fp = fingerprint();
      if (fp === lastFp) return null;
      lastFp = fp;
      last = buildMultiState(cfg, src, selectedId, nowMs);
      return last;
    },
    current: () => last,
  };
}
