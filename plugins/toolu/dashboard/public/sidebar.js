// Sidebar: the list of discovered projects with a status dot, plan progress, and
// a live-agent badge. Selecting a project lifts its id to the app. React via htm
// (no JSX/build); pure presentation over the cheap ProjectSummary[].

import React from "react";
import htm from "htm";

import { PROJECT_DOT, planTotals } from "/static/view-model.js";

const html = htm.bind(React.createElement);

function Dot({ dot }) {
  const meta = PROJECT_DOT[dot] ?? PROJECT_DOT.idle;
  const live = dot === "running";
  return html`<span
    class=${`inline-block h-2.5 w-2.5 rounded-full ${live ? "pulse" : ""}`}
    style=${{ backgroundColor: meta.color }}
    title=${meta.label}
  ></span>`;
}

function ProjectRow({ summary, active, onSelect }) {
  const totals = planTotals(summary.planCounts);
  const live = summary.dot === "running";
  return html`<button
    type="button"
    onClick=${() => onSelect(summary.id)}
    class=${`w-full text-left px-3 py-2.5 rounded-lg border transition-colors ${
      active ? "bg-panel2 border-accent/40" : "bg-transparent border-transparent hover:bg-panel2/60"
    }`}
  >
    <div class="flex items-center gap-2">
      <${Dot} dot=${summary.dot} />
      <span class="truncate font-medium text-sm">${summary.label}</span>
    </div>
    <div class="mt-1.5 flex items-center gap-2 text-xs text-muted">
      <span class="font-mono">${summary.planCounts.green}/${totals.total}</span>
      <div class="flex-1 h-1 rounded bg-line overflow-hidden">
        <div class="h-full bg-ok" style=${{ width: `${totals.donePct}%` }}></div>
      </div>
      ${summary.agentsSpawned > 0 &&
      html`<span class="font-mono text-[10px] px-1.5 py-0.5 rounded bg-line/60" title="agents spawned">⛛ ${summary.agentsSpawned}</span>`}
      ${live && html`<span class="text-accent text-[10px]" title="running">●</span>`}
    </div>
  </button>`;
}

export function Sidebar({ projects, selectedId, onSelect }) {
  return html`<aside class="w-72 shrink-0 border-r border-line bg-panel/60 flex flex-col min-h-0">
    <div class="px-4 py-3 text-[11px] uppercase tracking-wider text-muted border-b border-line">
      Projects · ${projects.length}
    </div>
    <div class="flex-1 overflow-auto p-2 space-y-1">
      ${projects.length === 0
        ? html`<p class="px-3 py-6 text-sm text-muted">No active projects. Add roots with <span class="font-mono">dashboard-config</span>.</p>`
        : projects.map(
            (p) => html`<${ProjectRow} key=${p.id} summary=${p} active=${p.id === selectedId} onSelect=${onSelect} />`,
          )}
    </div>
  </aside>`;
}
