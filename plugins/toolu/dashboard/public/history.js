// History lane: the persisted past-runs list for the selected project. Sessions
// are grouped by day (Today / Yesterday / date, newest first); each row shows a
// short session id, time, agent count, and a status dot. Clicking a row fetches
// /api/session?project=&session= and renders THAT past run by reusing #117's live
// Tree component (so a past run looks exactly like the live lane). Empty and
// fetch-error states are explicit — a failed fetch never blanks the view.

import React, { useState } from "react";
import htm from "htm";

import { Tree } from "/static/tree.js";
import { groupSessionsByDay, sessionDate, sessionDotColor } from "/static/view-model.js";

const html = htm.bind(React.createElement);

/** A short, glanceable session id (first 8 chars) for the row label. */
function shortId(sessionId) {
  return sessionId.length > 8 ? sessionId.slice(0, 8) : sessionId;
}

/** Time-of-day for a row, or "—" when the session has no timestamp. */
function rowTime(d) {
  return d ? d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" }) : "—";
}

/** One session row; expands in place to its past tree when opened. */
function SessionRow({ projectId, summary, open, onToggle }) {
  const [state, setState] = useState({ status: "idle", detail: null });
  const d = sessionDate(summary);

  const onClick = async () => {
    onToggle(summary.sessionId);
    if (open || state.status === "loaded") return; // collapsing or already loaded
    setState({ status: "loading", detail: null });
    try {
      const qs = `project=${encodeURIComponent(projectId)}&session=${encodeURIComponent(summary.sessionId)}`;
      const res = await fetch(`/api/session?${qs}`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const detail = await res.json();
      setState({ status: "loaded", detail });
    } catch {
      setState({ status: "error", detail: null });
    }
  };

  return html`<div class="border-b border-line/60 last:border-b-0">
    <button
      type="button"
      onClick=${onClick}
      class="w-full text-left px-3 py-2 flex items-center gap-2 hover:bg-panel2/60"
    >
      <span class="h-2 w-2 rounded-full shrink-0" style=${{ backgroundColor: sessionDotColor(summary) }}></span>
      <span class="font-mono text-xs">${shortId(summary.sessionId)}</span>
      <span class="text-xs text-muted">${rowTime(d)}</span>
      <span class="flex-1"></span>
      <span class="font-mono text-[10px] text-muted" title="agents">⛛ ${summary.agentCount}</span>
      <span class="text-muted text-[10px]">${open ? "▾" : "▸"}</span>
    </button>
    ${open &&
    html`<div class="px-2 pb-2">
      ${state.status === "loading" &&
      html`<p class="px-3 py-3 text-xs text-muted">Loading run…</p>`}
      ${state.status === "error" &&
      html`<p class="px-3 py-3 text-xs text-bad">Could not load this run. Try again.</p>`}
      ${state.status === "loaded" &&
      html`<${Tree} agents=${state.detail.tree} activity=${state.detail.summary} />`}
    </div>`}
  </div>`;
}

/** The history lane for the selected project. `sessions` is SessionSummary[];
 *  `onPick` (optional) is notified with the session id whenever a row toggles. */
export function History({ projectId, sessions, onPick }) {
  const [openId, setOpenId] = useState(null);
  const list = sessions ?? [];

  const onToggle = (sessionId) => {
    const next = openId === sessionId ? null : sessionId;
    setOpenId(next);
    if (onPick) onPick(next);
  };

  return html`<section id="history-lane" class="rounded-lg bg-panel border border-line" data-testid="history">
    <header class="px-3 py-2 flex items-center justify-between border-b border-line">
      <h3 class="text-[11px] uppercase tracking-wider text-muted">History</h3>
      <span class="text-xs text-muted">${list.length} ${list.length === 1 ? "run" : "runs"}</span>
    </header>
    ${list.length === 0
      ? html`<p class="px-3 py-6 text-sm text-muted">No past runs recorded for this project yet.</p>`
      : groupSessionsByDay(list).map(
          (g) => html`<div key=${g.label}>
            <div class="px-3 pt-3 pb-1 text-[10px] uppercase tracking-wider text-muted/80">${g.label}</div>
            ${g.rows.map(
              (s) => html`<${SessionRow}
                key=${s.sessionId}
                projectId=${projectId}
                summary=${s}
                open=${openId === s.sessionId}
                onToggle=${onToggle}
              />`,
            )}
          </div>`,
        )}
  </section>`;
}
