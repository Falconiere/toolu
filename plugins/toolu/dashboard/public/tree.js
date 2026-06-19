// Activity lane: the live agent/sub-agent spawn tree for the selected project.
// Flattened preorder with indentation; running nodes pulse, errors/stale are
// tagged. This is the "what's actually running" view that the ledger can't show.

import React from "react";
import htm from "htm";

import { AGENT_STATUS, formatDuration, treeRows } from "/static/view-model.js";

const html = htm.bind(React.createElement);

function StatusPill({ status }) {
  const meta = AGENT_STATUS[status] ?? AGENT_STATUS.running;
  return html`<span
    class="text-[10px] font-medium px-1.5 py-0.5 rounded"
    style=${{ color: meta.color, backgroundColor: meta.color + "1f" }}
  >${meta.label}</span>`;
}

function Row({ node, depth }) {
  const meta = AGENT_STATUS[node.status] ?? AGENT_STATUS.running;
  return html`<div class="fade-in flex items-center gap-2 py-1.5 pr-2" style=${{ paddingLeft: `${depth * 18 + 8}px` }}>
    <span
      class=${`h-2 w-2 rounded-full shrink-0 ${node.status === "running" ? "pulse" : ""}`}
      style=${{ backgroundColor: meta.color }}
    ></span>
    <span class="text-sm truncate flex-1" title=${node.label}>${node.label || "(agent)"}</span>
    ${node.agentType &&
    html`<span class="font-mono text-[10px] text-muted hidden sm:inline">${node.agentType}</span>`}
    <span class="font-mono text-[11px] text-muted w-16 text-right">${formatDuration(node.durationMs)}</span>
    <${StatusPill} status=${node.status} />
  </div>`;
}

function Summary({ activity }) {
  const item = (label, n, color) =>
    html`<span class="text-xs" style=${{ color }}>${n} <span class="text-muted">${label}</span></span>`;
  return html`<div class="flex items-center gap-3">
    ${item("total", activity.total, "#e6e9ef")}
    ${activity.running > 0 && item("running", activity.running, "#4aa3ff")}
    ${activity.errored > 0 && item("errored", activity.errored, "#e74c3c")}
    ${activity.stale > 0 && item("stale", activity.stale, "#f1c40f")}
  </div>`;
}

export function Tree({ agents, activity }) {
  const rows = treeRows(agents);
  return html`<section class="rounded-lg bg-panel border border-line">
    <header class="px-3 py-2 flex items-center justify-between border-b border-line">
      <h3 class="text-[11px] uppercase tracking-wider text-muted">Live agents</h3>
      <${Summary} activity=${activity} />
    </header>
    <div class="divide-y divide-line/60">
      ${rows.length === 0
        ? html`<p class="px-3 py-6 text-sm text-muted">No agent activity captured for this project.</p>`
        : rows.map((r) => html`<${Row} key=${r.node.agentId} node=${r.node} depth=${r.depth} />`)}
    </div>
  </section>`;
}
