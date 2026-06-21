// Dashboard root. Subscribes to /api/events (SSE) — re-subscribing with
// ?project=<id> when the selection changes — and renders the sidebar plus the
// selected project's two lanes (plan kanban + live agent tree). React via htm,
// no JSX/build; CDN-loaded per the s9 plan deviation.

import React, { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import htm from "htm";

import { Sidebar } from "/static/sidebar.js";
import { Board } from "/static/board.js";
import { Tree } from "/static/tree.js";
import { History } from "/static/history.js";
import { planTotals } from "/static/view-model.js";

const html = htm.bind(React.createElement);

const EMPTY = { projects: [], selected: null, serverTime: null };

/** Subscribe to SSE; re-subscribe when the selection changes. */
function useDashboard() {
  const [state, setState] = useState(EMPTY);
  const [conn, setConn] = useState("connecting");
  const [selectedId, setSelectedId] = useState(null);
  const esRef = useRef(null);

  useEffect(() => {
    const qs = selectedId ? `?project=${encodeURIComponent(selectedId)}` : "";
    const es = new EventSource(`/api/events${qs}`);
    esRef.current = es;
    es.addEventListener("open", () => setConn("live"));
    es.addEventListener("error", () => setConn("reconnecting"));
    es.addEventListener("state", (ev) => {
      setConn("live");
      try {
        setState(JSON.parse(ev.data));
      } catch {
        /* ignore a malformed frame; the next one will be clean */
      }
    });
    return () => es.close();
  }, [selectedId]);

  return { state, conn, selectedId, setSelectedId };
}

function ConnPill({ conn }) {
  const color = conn === "live" ? "#2ecc71" : conn === "reconnecting" ? "#e74c3c" : "#8b94a7";
  return html`<span class="inline-flex items-center gap-1.5 text-xs text-muted">
    <span class="h-2 w-2 rounded-full" style=${{ backgroundColor: color }}></span>${conn}
  </span>`;
}

function TopBar({ conn }) {
  return html`<header class="h-12 shrink-0 flex items-center justify-between px-4 border-b border-line bg-panel">
    <div class="flex items-center gap-2">
      <span class="font-semibold tracking-tight">toolu</span>
      <span class="text-muted text-sm">execution dashboard</span>
    </div>
    <${ConnPill} conn=${conn} />
  </header>`;
}

function ProjectHeader({ detail }) {
  const counts = {
    pending: detail.plan.steps.filter((s) => s.status === "pending").length,
    running: detail.plan.steps.filter((s) => s.status === "running").length,
    red: detail.plan.steps.filter((s) => s.status === "red").length,
    green: detail.plan.steps.filter((s) => s.status === "green").length,
  };
  const totals = planTotals(counts);
  const planDoc = detail.plan.ledger?.plan_doc ?? "—";
  return html`<div class="flex items-center justify-between flex-wrap gap-2 mb-3">
    <div>
      <h2 class="text-lg font-semibold">${detail.plan.ledger?.branch ?? detail.id}</h2>
      <div class="font-mono text-xs text-muted">${planDoc}</div>
    </div>
    <div class="flex items-center gap-3">
      <span class="font-mono text-sm">${counts.green}/${totals.total} done</span>
      <div class="w-40 h-1.5 rounded bg-line overflow-hidden">
        <div class="h-full bg-ok" style=${{ width: `${totals.donePct}%` }}></div>
      </div>
    </div>
  </div>`;
}

function ProjectView({ detail }) {
  return html`<div id="project-view" class="p-4 space-y-4">
    <${ProjectHeader} detail=${detail} />
    <${Board} plan=${detail.plan} />
    <${Tree} agents=${detail.agents} activity=${detail.activity} />
    <${History} projectId=${detail.id} sessions=${detail.sessions} />
  </div>`;
}

function Empty() {
  return html`<div class="h-full flex items-center justify-center text-muted text-sm">
    Select a project to see its plan and live agents.
  </div>`;
}

function App() {
  const { state, conn, selectedId, setSelectedId } = useDashboard();
  const sel = state.selected;
  return html`<div class="h-screen flex flex-col">
    <${TopBar} conn=${conn} />
    <div class="flex flex-1 min-h-0">
      <${Sidebar} projects=${state.projects} selectedId=${sel?.id ?? selectedId} onSelect=${setSelectedId} />
      <main class="flex-1 min-w-0 overflow-auto">
        ${sel ? html`<${ProjectView} detail=${sel} />` : html`<${Empty} />`}
      </main>
    </div>
  </div>`;
}

createRoot(document.getElementById("root")).render(html`<${App} />`);
