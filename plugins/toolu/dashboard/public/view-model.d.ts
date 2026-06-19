// Type declarations for the browser-served view-model.js, so the TS tests (and
// any TS consumer) get types without transpiling the runtime module. The browser
// loads only the .js; this .d.ts is compile-time only.

import type { AgentNode } from "../activity/source.ts";
import type { DerivedStep } from "../state.ts";

export interface StatusMeta {
  label: string;
  color: string;
}
export const AGENT_STATUS: Record<string, StatusMeta>;
export const PROJECT_DOT: Record<string, StatusMeta>;

export interface TreeRow {
  node: AgentNode;
  depth: number;
}
export function treeRows(agents: AgentNode[]): TreeRow[];

export function formatDuration(ms: number | null | undefined): string;

export interface PlanCounts {
  pending: number;
  running: number;
  red: number;
  green: number;
}
export function planTotals(counts: PlanCounts): { total: number; donePct: number };

export const COLUMNS: { key: keyof PlanCounts; title: string; status: string }[];
export function columnize(steps: DerivedStep[]): Record<keyof PlanCounts, DerivedStep[]>;

export function hasLiveActivity(
  activity: { running?: number; stale?: number } | null | undefined,
): boolean;
