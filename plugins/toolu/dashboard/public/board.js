// Plan lane: the four-column kanban (To Do / Running / Blocked / Done) over the
// selected project's derived ledger steps. A running card pulses; stale/stuck
// carry an amber badge. React via htm. Verified truth — mirrors the ledger.

import React from "react";
import htm from "htm";

import { COLUMNS, columnize } from "/static/view-model.js";

const html = htm.bind(React.createElement);

const BORDER = {
  pending: "border-l-muted",
  running: "border-l-accent",
  red: "border-l-bad",
  green: "border-l-ok",
};

function Card({ step }) {
  const amber = step.stale || step.stuck;
  const border = amber ? "border-l-warn" : BORDER[step.status] ?? "border-l-muted";
  const badge = step.stale ? "↻ stale" : step.stuck ? "⚠ stuck" : null;
  return html`<article
    class=${`fade-in rounded-md bg-panel2 border-l-[3px] ${border} px-3 py-2 ${
      step.status === "running" ? "pulse" : ""
    }`}
  >
    <div class="font-mono text-[11px] text-muted">${step.id}</div>
    <div class="text-sm mt-0.5">
      ${step.title}
      ${badge && html`<span class="ml-1.5 text-[10px] text-warn font-medium">${badge}</span>`}
    </div>
    ${step.activity &&
    html`<div class="mt-1 text-xs text-muted truncate" title=${step.activity}>${step.activity}</div>`}
  </article>`;
}

function Column({ title, status, steps }) {
  return html`<section class="flex flex-col min-h-0 rounded-lg bg-panel border border-line">
    <h3 class="px-3 py-2 text-[11px] uppercase tracking-wider text-muted flex items-center justify-between border-b border-line">
      <span>${title}</span>
      <span class="font-mono text-ink/70">${steps.length}</span>
    </h3>
    <div class="flex-1 overflow-auto p-2 space-y-2">
      ${steps.map((s) => html`<${Card} key=${s.id} step=${s} />`)}
    </div>
  </section>`;
}

export function Board({ plan }) {
  const cols = columnize(plan.steps);
  return html`<div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-3">
    ${COLUMNS.map((c) => html`<${Column} key=${c.key} title=${c.title} status=${c.status} steps=${cols[c.key]} />`)}
  </div>`;
}
