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

/** Human-readable duration: "—" when null, "1.4s", "2m 05s", "1h 03m". */
export function formatDuration(ms) {
  if (ms === null || ms === undefined || Number.isNaN(ms)) return "—";
  if (ms < 0) ms = 0;
  const totalSec = Math.floor(ms / 1000);
  if (totalSec < 60) return `${(ms / 1000).toFixed(1)}s`;
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
