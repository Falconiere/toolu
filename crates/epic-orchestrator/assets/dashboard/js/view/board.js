import { liveRows, shortKey, waveGroups, waveLabel } from "../logic/board.js";
import {
  agentState,
  linkUrl,
  liveTitle,
  phaseLine,
  statusTitle,
  waveLine,
} from "../logic/status.js";
import { $, changed, element, mark, redraw } from "./dom.js";

// Column labels; a named column drops out on narrower screens, cell and label alike.
const COLUMNS = [
  ["STATE"],
  ["KEY"],
  ["ISSUE"],
  ["HOST", "host"],
  ["PLAN", "plan"],
  ["PHASE", "phase"],
  ["ATTN"],
  [""],
];

// The operator's own open or closed choice per epic and wave; it outlives every rebuild.
const toggled = new Map();

const focusId = (node) => node.dataset.issue || node.dataset.wave || node.dataset.link;

function cell(className, column, value) {
  const node = element("span", className, value);
  node.dataset.col = column;
  return node;
}

function external(className, href, { label, name, id }) {
  const link = element("a", className, label);
  link.href = href;
  link.target = "_blank";
  link.rel = "noopener noreferrer";
  // Apart from the row's own key, so a rebuild returns focus to the link, not the row.
  link.dataset.link = `link ${id}`;
  link.setAttribute("aria-label", name);
  return link;
}

// The second line of a row, only when it says more than the state already does.
function detail(issue, live, peer) {
  if (peer) return null;
  if (!issue.stage && !issue.closed && issue.open_blockers?.length) {
    return `blocked by ${issue.open_blockers.join(", ")}`;
  }
  return statusTitle(live, issue.key) ? liveTitle(live.host || issue.host, live.title) : null;
}

function planCell(issue) {
  const node = cell("issue-plan", "plan");
  const plan = issue.plan_summary;
  if (!plan || plan.error) {
    node.append(element("span", "", plan?.error ? "UNREADABLE" : "—"));
    return node;
  }
  node.append(element("span", "", `${plan.green ?? 0}/${plan.total ?? 0}`));
  if (plan.total > 0) {
    const bar = element("progress");
    bar.max = plan.total;
    bar.value = Math.min(plan.green ?? 0, plan.total);
    bar.setAttribute("aria-label", "Plan steps green");
    node.append(bar);
  }
  return node;
}

function issueLine(issue, { liveByKey, attention, peer }) {
  const live = liveByKey.get(issue.key);
  const title = issue.title || "Untitled issue";
  const button = element("button", "issue-row");
  button.type = "button";
  button.dataset.issue = issue.key;
  button.setAttribute("aria-label", `Open ${issue.key}: ${title}`);
  const identity = element("span", "issue-identity");
  identity.append(element("span", "issue-title", title));
  const more = detail(issue, live, peer);
  if (more) identity.append(element("small", "issue-live", more));
  const count = attention.filter((key) => key === issue.key).length;
  const host = issue.host ? `${issue.host} · ${issue.model || "model unknown"}` : "—";
  button.append(
    mark(agentState(issue, live?.agent_status)),
    element("span", "issue-key", shortKey(issue.key)),
    identity,
    cell("issue-host", "host", host),
    planCell(issue),
    cell("issue-phase", "phase", phaseLine(issue, issue.wave)),
    element("span", "issue-attention", count ? `▲ ${count}` : ""),
  );
  const line = element("div", "issue-line");
  line.append(button);
  const href = linkUrl(issue.url);
  if (href) {
    const name = `Open ${issue.key} in its tracker`;
    line.append(external("issue-link", href, { label: "↗", name, id: issue.key }));
  }
  return line;
}

function table(rows, context) {
  const grid = element("div", "issue-table");
  grid.append(...rows.map((row) => issueLine(row, context)));
  return grid;
}

function tableHead() {
  const grid = element("div", "issue-table issue-head");
  for (const [label, column] of COLUMNS) {
    const node = element("span", "column-label", label);
    if (column) node.dataset.col = column;
    grid.append(node);
  }
  return grid;
}

function waveSection(board, group, context) {
  const id = `${board.key} ${group.wave}`;
  const section = element("details", group.current ? "wave wave-current" : "wave");
  section.open = toggled.get(id) ?? group.open;
  const summary = element("summary", "", waveLabel(group));
  summary.dataset.wave = id;
  section.append(summary, table(group.rows, context));
  return section;
}

function epicHeader(board, slot) {
  const header = element("header", "epic-header");
  const identity = element("div", "epic-identity");
  identity.append(element("span", "eyebrow", `EPIC ${shortKey(board.key)}`));
  if (board.paused) identity.append(mark({ kind: "paused", label: "‖ PAUSED" }));
  const href = linkUrl(board.url);
  if (href) {
    const name = `Open epic ${board.key} in its tracker`;
    identity.append(external("epic-link", href, { label: "TRACKER ↗", name, id: board.key }));
  }
  identity.append(element("h2", "", board.title || board.key));
  slot.dataset.epic = board.key;
  header.append(identity, element("p", "wave-line", waveLine(board)), slot);
  return header;
}

// What a row shows besides its own record: live agent status and attention, by key.
function rowContext(live, attention, peer) {
  const items = (live?.items ?? []).map(({ key, agent_status, host, title }) => ({
    key,
    agent_status,
    host,
    title,
  }));
  return {
    liveByKey: new Map(items.map((item) => [item.key, item])),
    items,
    attention: (attention.items ?? []).map((item) => item.key),
    peer: Boolean(peer),
  };
}

// The selected epic: its header, then one collapsible group per wave.
export function renderEpic(board, live, attention, peer) {
  const target = $("board");
  const context = rowContext(live, attention, peer);
  if (!changed(target, [board ?? null, context.items, context.attention, context.peer])) return;
  if (!board) {
    target.replaceChildren(element("p", "empty", "No epic selected."));
    return;
  }
  // The rebuilt header takes its old action slot back, so a button mid-click survives.
  const old = target.querySelector(".epic-control-slot");
  const slot = old?.dataset.epic === board.key ? old : element("div", "epic-control-slot");
  const section = element("section", "epic");
  section.append(epicHeader(board, slot));
  const groups = waveGroups(board);
  if (groups.length) section.append(tableHead());
  else section.append(element("p", "empty", "No issues in this epic yet."));
  section.append(...groups.map((group) => waveSection(board, group, context)));
  redraw(target, focusId, () => target.replaceChildren(section));
}

// The overview's list: every running or merging worker, across epics.
export function renderLive(boards, live, attention, peer) {
  const target = $("live");
  const rows = liveRows(boards).map(({ row }) => row);
  const context = rowContext(live, attention, peer);
  if (!changed(target, [rows, context.items, context.attention, context.peer])) return;
  redraw(target, focusId, () =>
    target.replaceChildren(
      ...(rows.length
        ? [tableHead(), table(rows, context)]
        : [element("p", "empty", "No workers running.")]),
    ),
  );
}

export function showBoardMessage(message) {
  for (const id of ["board", "live"]) {
    const target = $(id);
    delete target.dataset.signature;
    target.replaceChildren(element("p", "empty", message));
  }
}

export function bindBoard() {
  $("board").addEventListener("click", (event) => {
    const summary = event.target.closest?.("summary[data-wave]");
    if (summary) toggled.set(summary.dataset.wave, !summary.parentElement.open);
  });
}
