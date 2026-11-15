import { attentionOpen } from "./control.js";
import { DONE_STAGES, LIVE_STAGES, epicComplete, hostState, issueRows } from "./status.js";

const count = (value) => (Number.isFinite(value) && value > 0 ? value : 0);
const joined = (parts) => parts.filter(Boolean).join(" · ");

// `#870` for a key that ends in an issue number; any other key stays whole.
export function shortKey(key) {
  const number = /-(\d+)$/.exec(key ?? "");
  return number ? `#${number[1]}` : (key ?? "");
}

// An epic the current documents do not hold is not a view.
export const resolveView = (view, epics) =>
  view.kind !== "epic" || (epics ?? []).some((epic) => epic.key === view.key)
    ? view
    : { kind: "overview" };

function epicEntry(epic) {
  const complete = epicComplete(epic);
  const live = count(epic.live);
  const queued = count(epic.queued);
  const attention = count(epic.attention_count);
  return {
    view: { kind: "epic", key: epic.key },
    label: `EPIC ${shortKey(epic.key)}`,
    title: epic.title || epic.key,
    meta: complete
      ? "✓ COMPLETE"
      : joined([
          epic.paused && "‖ PAUSED",
          `WAVE ${epic.current_wave ?? "—"}/${epic.waves_total ?? "—"}`,
          live && `${live} LIVE`,
          queued && `${queued} QUEUED`,
        ]),
    badge: attention ? `▲ ${attention}` : "",
    complete,
  };
}

// The sidebar: overview, unfinished epics, complete epics, hosts, journal.
export function sidebarEntries(docs, now = Date.now()) {
  const open = (docs?.attention?.items ?? []).filter(attentionOpen).length;
  const hosts = docs?.hosts?.items ?? [];
  const rotation = hosts.filter((host) => hostState(host, now).kind === "rotation").length;
  const epics = (docs?.epics?.items ?? []).map(epicEntry);
  return [
    { view: { kind: "overview" }, label: "Overview", badge: open ? `▲ ${open}` : "" },
    ...epics.filter((entry) => !entry.complete),
    ...epics.filter((entry) => entry.complete),
    {
      view: { kind: "hosts" },
      label: "Hosts",
      badge: hosts.length ? `${rotation}/${hosts.length}` : "",
    },
    { view: { kind: "journal" }, label: "Journal", badge: "" },
  ];
}

function waveGroup(wave, rows, current) {
  const tally = (wanted) => rows.filter(wanted).length;
  const isCurrent = wave != null && wave === current;
  return {
    wave,
    current: isCurrent,
    // Rows the graph gives no wave have no summary to reopen them from.
    open: isCurrent || wave == null,
    // By stage, not by the agent's badge: an agent reports `done` after every turn.
    done: tally((row) => DONE_STAGES.includes(row.stage) || row.closed),
    live: tally((row) => LIVE_STAGES.includes(row.stage)),
    queued: tally((row) => !row.stage && !row.closed),
    total: rows.length,
    rows,
  };
}

// The board's rows by graph wave, then one last group for rows without a wave.
export function waveGroups(board) {
  const rows = issueRows(board);
  const byKey = new Map(rows.map((row) => [row.key, row]));
  const groups = (board.graph?.waves ?? []).map(({ wave, issues }) =>
    waveGroup(
      wave,
      (issues ?? []).map((key) => byKey.get(key)).filter(Boolean),
      board.current_wave,
    ),
  );
  const placed = new Set(groups.flatMap((group) => group.rows.map((row) => row.key)));
  const rest = rows.filter((row) => !placed.has(row.key));
  return rest.length ? [...groups, waveGroup(null, rest, board.current_wave)] : groups;
}

export function waveLabel(group) {
  if (group.wave == null) {
    return `UNSCHEDULED · ${group.total} ${group.total === 1 ? "ISSUE" : "ISSUES"}`;
  }
  return joined([
    `WAVE ${group.wave}`,
    group.current && "CURRENT",
    group.live && `${group.live} LIVE`,
    group.queued && `${group.queued} QUEUED`,
    group.done && `${group.done} DONE`,
  ]);
}

// The running and merging workers of every board, each with its board.
export const liveRows = (boards) =>
  (boards ?? []).flatMap((board) =>
    issueRows(board)
      .filter((row) => LIVE_STAGES.includes(row.stage))
      .map((row) => ({ board, row })),
  );
