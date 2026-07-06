// Pure, framework-free view-model helpers for the dashboard UI. No React, no DOM,
// no imports — so the rendering logic (tree flattening, durations, status/dot
// mapping, plan progress) is unit-tested directly on a real MultiDashboardState
// payload, while the React components stay thin wrappers over these.

/** Meta for an agent status: label + a color token (consumed as a CSS var / class). */
export const AGENT_STATUS = {
  running: { label: "running", color: "#4aa3ff" },
  done: { label: "done", color: "#2ecc71" },
  error: { label: "error", color: "#e74c3c" },
  stale: { label: "stale", color: "#f1c40f" },
};

/** Meta for a project sidebar dot. */
export const PROJECT_DOT = {
  running: { label: "running", color: "#4aa3ff" },
  blocked: { label: "blocked", color: "#e74c3c" },
  done: { label: "done", color: "#2ecc71" },
  idle: { label: "idle", color: "#8b94a7" },
};

/** Flatten an agent tree (preorder) into rows tagged with depth, for indented rendering. */
export function treeRows(agents) {
  const rows = [];
  const walk = (nodes, depth) => {
    for (const node of nodes ?? []) {
      rows.push({ node, depth });
      if (node.children && node.children.length > 0) walk(node.children, depth + 1);
    }
  };
  walk(agents, 0);
  return rows;
}

/** Human-readable duration: "—" when null/invalid, "1.4s", "2m 05s", "1h 03m". */
export function formatDuration(ms) {
  if (ms === null || ms === undefined || Number.isNaN(ms)) return "—";
  if (ms < 0) return "—"; // negative duration is corrupt upstream data, not "0.0s"
  const totalSec = Math.floor(ms / 1000);
  // Truncate (not round) the tenths so 59999ms reads "59.9s", never "60.0s".
  if (totalSec < 60) return `${(Math.floor(ms / 100) / 10).toFixed(1)}s`;
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  if (min < 60) return `${min}m ${String(sec).padStart(2, "0")}s`;
  const hr = Math.floor(min / 60);
  return `${hr}h ${String(min % 60).padStart(2, "0")}m`;
}

/** Plan progress: total step count + percent done (green/total, 0 when empty). */
export function planTotals(counts) {
  const c = counts ?? { pending: 0, running: 0, red: 0, green: 0 };
  const total = c.pending + c.running + c.red + c.green;
  const donePct = total === 0 ? 0 : Math.round((c.green / total) * 100);
  return { total, donePct };
}

/** A session's status -> a dot color (running > errored > done). */
export function sessionDotColor(s) {
  if (s?.running) return AGENT_STATUS.running.color;
  if (s?.errored) return AGENT_STATUS.error.color;
  return AGENT_STATUS.done.color;
}

/** A session's representative instant (end, else start) as a Date, or null. */
export function sessionDate(s) {
  const iso = s?.endedAt ?? s?.startedAt;
  if (!iso) return null;
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? null : d;
}

/** Day-bucket label for a date relative to `now`: "Today", "Yesterday", or a date. */
export function dayLabel(d, now = new Date()) {
  if (!d) return "Unknown";
  const startOf = (x) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diffDays = Math.round((startOf(now) - startOf(d)) / 86_400_000);
  if (diffDays <= 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
}

/** Group sessions into ordered day buckets (newest day first, newest row first),
 *  each `{ label, rows }`. Pure: no DOM/React, unit-tested on real summaries. */
export function groupSessionsByDay(sessions, now = new Date()) {
  const ordered = [...(sessions ?? [])].sort((a, b) => {
    const da = sessionDate(a);
    const db = sessionDate(b);
    return (db ? db.getTime() : 0) - (da ? da.getTime() : 0);
  });
  const groups = [];
  const byLabel = new Map();
  for (const s of ordered) {
    const label = dayLabel(sessionDate(s), now);
    let bucket = byLabel.get(label);
    if (!bucket) {
      bucket = { label, rows: [] };
      byLabel.set(label, bucket);
      groups.push(bucket);
    }
    bucket.rows.push(s);
  }
  return groups;
}

/** The four kanban columns, in display order. */
export const COLUMNS = [
  { key: "pending", title: "To Do", status: "pending" },
  { key: "running", title: "Running", status: "running" },
  { key: "red", title: "Blocked", status: "red" },
  { key: "green", title: "Done", status: "green" },
];

/** Group a plan's derived steps into the four columns by status. */
export function columnize(steps) {
  const cols = { pending: [], running: [], red: [], green: [] };
  for (const s of steps ?? []) {
    if (s.status === "running") cols.running.push(s);
    else if (s.status === "red") cols.red.push(s);
    else if (s.status === "green") cols.green.push(s);
    else cols.pending.push(s);
  }
  return cols;
}
