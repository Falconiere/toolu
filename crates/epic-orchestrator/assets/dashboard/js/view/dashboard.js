import {
  agentState,
  hostNote,
  hostState,
  issueRows,
  liveTitle,
  phaseLine,
  statusTitle,
  tiles,
  waveLine,
} from "../logic/status.js";
import { $, element, mark, text } from "./dom.js";

export function renderMachines(fleet, selected, select) {
  const nav = $("machines");
  const focused = document.activeElement?.dataset?.machine;
  nav.replaceChildren();
  for (const machine of fleet.machines ?? []) {
    const button = element("button", "machine");
    button.type = "button";
    button.dataset.machine = machine.name;
    button.setAttribute("aria-pressed", String((machine.local ? null : machine.name) === selected));
    button.append(
      element("span", "machine-kind", machine.local ? "LOCAL" : "PEER"),
      element(
        "span",
        "machine-name",
        `· ${machine.local ? machine.hostname || "THIS MACHINE" : machine.name.toUpperCase()}`,
      ),
    );
    button.addEventListener("click", () => select(machine.local ? null : machine.name));
    nav.append(button);
  }
  if (focused) {
    [...nav.querySelectorAll("button[data-machine]")]
      .find((button) => button.dataset.machine === focused)
      ?.focus({ preventScroll: true });
  }
}

function renderTiles(docs, sampledAt, now) {
  const elapsed = Math.max(0, now - sampledAt);
  const attention = {
    items: (docs.attention.items ?? []).map((item) => ({
      ...item,
      age_ms: (item.age_ms ?? 0) + elapsed,
    })),
  };
  const totals = tiles(docs.boards, docs.hosts, attention, now);
  for (const [id, value] of Object.entries({
    "tile-workers": totals.workers,
    "tile-workers-note": totals.workerNote,
    "tile-attention": totals.attention,
    "tile-attention-note": totals.attentionNote,
    "tile-hosts": totals.hosts,
    "tile-plans": totals.plans,
    "tile-plans-note": totals.planNote,
  }))
    text(id, value);
  return attention;
}

function epicHeader(board) {
  const header = element("header", "epic-header");
  const identity = element("div", "epic-identity");
  identity.append(element("span", "eyebrow", `EPIC #${board.key.split("-").at(-1)}`));
  if (board.paused) identity.append(mark({ kind: "paused", label: "‖ PAUSED" }));
  identity.append(element("h3", "", board.title || board.key));
  header.append(identity, element("p", "wave-line", waveLine(board)));
  const slot = element("div", "epic-control-slot");
  slot.dataset.epic = board.key;
  header.append(slot);
  return header;
}

function rowLine(issue, live, peer) {
  if (peer) return "live status on peer";
  if (!issue.stage && issue.open_blockers?.length) {
    return `blocked by ${issue.open_blockers.join(", ")}`;
  }
  const state = agentState(issue, live?.agent_status);
  return statusTitle(live, issue.key)
    ? `${state.kind} · ${liveTitle(live.host || issue.host, live.title)}`
    : state.kind;
}

function planCell(issue) {
  const cell = element("span", "issue-plan");
  const plan = issue.plan_summary;
  if (!plan || plan.error) {
    cell.append(element("span", "", plan?.error ? "UNREADABLE" : "—"));
    return cell;
  }
  cell.append(element("span", "", `${plan.green ?? 0}/${plan.total ?? 0}`));
  if (plan.total > 0) {
    const bar = element("progress");
    bar.max = plan.total;
    bar.value = Math.min(plan.green ?? 0, plan.total);
    bar.setAttribute("aria-label", "Plan steps green");
    cell.append(bar);
  }
  return cell;
}

function issueButton(issue, live, attention, peer) {
  const title = issue.title || "Untitled issue";
  const button = element("button", "issue-row");
  button.type = "button";
  button.dataset.issue = issue.key;
  button.setAttribute("aria-label", `Open ${issue.key}: ${title}`);
  button.append(mark(agentState(issue, live?.agent_status)));
  const identity = element("span", "issue-identity");
  identity.append(element("strong", "", issue.key), element("span", "issue-title", title));
  identity.append(element("small", "issue-live", rowLine(issue, live, peer)));
  button.append(identity);
  button.append(
    element(
      "span",
      "issue-host",
      issue.host ? `${issue.host} · ${issue.model || "model unknown"}` : "—",
    ),
  );
  button.append(planCell(issue));
  button.append(element("span", "issue-phase", phaseLine(issue, issue.wave)));
  const count = (attention.items ?? []).filter((item) => item.key === issue.key).length;
  button.append(element("span", "issue-attention", count ? `▲ ${count}` : "—"));
  return button;
}

function renderBoards(boards, live, attention, peer) {
  const target = $("board");
  // Attention keys carry `data-issue` too; restore focus only to a board row.
  const focused = target.contains(document.activeElement) && document.activeElement.dataset.issue;
  const scrolls = new Map(
    [...target.querySelectorAll(".table-scroll")].map((node) => [
      node.dataset.epic,
      node.scrollLeft,
    ]),
  );
  target.replaceChildren();
  if (!boards.length) {
    target.append(element("p", "empty", "No registered epics yet."));
    return;
  }
  const liveByKey = new Map((live?.items ?? []).map((item) => [item.key, item]));
  for (const board of boards) {
    const section = element("section", "epic");
    section.append(epicHeader(board));
    const scroll = element("div", "table-scroll");
    scroll.dataset.epic = board.key;
    const table = element("div", "issue-table");
    for (const label of ["AGENT", "ISSUE", "HOST", "PLAN", "PHASE", "ATTN"]) {
      table.append(element("span", "column-label", label));
    }
    for (const issue of issueRows(board)) {
      table.append(issueButton(issue, liveByKey.get(issue.key), attention, peer));
    }
    if (!board.graph?.issues?.length) {
      table.append(element("p", "empty", "No issues in this epic yet."));
    }
    scroll.append(table);
    section.append(scroll);
    target.append(section);
    scroll.scrollLeft = scrolls.get(board.key) ?? 0;
  }
  if (focused) {
    [...target.querySelectorAll("button[data-issue]")]
      .find((button) => button.dataset.issue === focused)
      ?.focus({ preventScroll: true });
  }
}

const HOST_NAMES = {
  claude: "Claude Code",
  codex: "Codex",
  cursor: "Cursor Agent",
  muse: "Muse Code",
};

function renderHosts(hosts, now) {
  const target = $("hosts");
  target.replaceChildren();
  for (const host of hosts.items ?? []) {
    const row = element("div", "host-row");
    const head = element("div", "host-head");
    head.append(
      element("strong", "", HOST_NAMES[host.host] || host.host),
      mark(hostState(host, now)),
    );
    const slot = element("div", "host-control-slot");
    slot.dataset.host = host.host;
    row.append(head, element("p", "host-meta", `${host.host} · ${host.running ?? 0} running`));
    row.append(element("p", "host-note", hostNote(host, now)), slot);
    target.append(row);
  }
  if (!target.children.length) target.append(element("p", "empty", "No hosts reported."));
}

function attentionKey(key, keys) {
  if (!key || !keys.has(key)) return element("span", "", key || "ENGINE");
  const button = element("button", "attention-key", key);
  button.type = "button";
  button.dataset.issue = key;
  button.setAttribute("aria-label", `Open ${key}`);
  return button;
}

function renderAttention(attention, boards) {
  const keys = new Set(
    boards.flatMap((board) => (board.graph?.issues ?? []).map((node) => node.key)),
  );
  const target = $("attention");
  const focused = target.contains(document.activeElement) && document.activeElement.dataset.issue;
  target.replaceChildren();
  const items = attention.items ?? [];
  text("attention-count", `${items.length} OPEN`);
  for (const item of items) {
    const row = element("li", "attention-row");
    const head = element("div", "attention-head");
    head.append(
      element("strong", "", `▲ ${(item.kind || "UNKNOWN").toUpperCase()}`),
      attentionKey(item.key, keys),
      element("time", "", `${Math.floor((item.age_ms ?? 0) / 60_000)}m`),
    );
    row.append(head, element("p", "", item.note || "Needs attention."));
    const slot = element("span", "attention-action-slot");
    Object.assign(slot.dataset, {
      kind: item.kind || "",
      key: item.key || "",
      state: item.state || "open",
    });
    row.append(slot);
    target.append(row);
  }
  if (!items.length)
    target.append(element("li", "empty", "Nothing needs you. Workers keep going."));
  if (focused) {
    [...target.querySelectorAll("button[data-issue]")]
      .find((button) => button.dataset.issue === focused)
      ?.focus({ preventScroll: true });
  }
}

function renderJournal(events, peer) {
  const target = $("journal");
  target.replaceChildren();
  if (peer) {
    target.append(element("li", "empty", "The event stream stays on the peer dashboard."));
    return;
  }
  for (const event of (events?.events ?? []).slice(-7).toReversed()) {
    const row = element("li", "journal-row");
    row.append(
      element("time", "", new Date(event.at).toLocaleTimeString([], { hour12: false })),
      element("span", "", `${event.name || event.kind} ${event.key || event.epic || ""}`.trim()),
    );
    target.append(row);
  }
  if (!target.children.length) target.append(element("li", "empty", "No recent events."));
}

export function renderDashboard(docs, live, peer, sampledAt, now = Date.now()) {
  const attention = renderTiles(docs, sampledAt, now);
  renderBoards(docs.boards, live, attention, peer);
  renderHosts(docs.hosts, now);
  renderAttention(attention, docs.boards);
  renderJournal(docs.events, peer);
  text(
    "sampled",
    `${peer ? "PEER · POLLED 5S" : "LIVE · POLLED 3S"} · SAMPLED ${new Date(sampledAt).toLocaleTimeString([], { hour12: false })}`,
  );
}

export function renderTick(docs, sampledAt, now = Date.now()) {
  const attention = renderTiles(docs, sampledAt, now);
  renderHosts(docs.hosts, now);
  renderAttention(attention, docs.boards);
}

export function renderUnavailable(message) {
  $("board").replaceChildren(element("p", "empty", message));
  for (const id of ["tile-workers", "tile-attention", "tile-hosts", "tile-plans"]) text(id, "—");
  for (const id of ["tile-workers-note", "tile-attention-note", "tile-plans-note"])
    text(id, message);
  text("attention-count", "0 OPEN");
  text("sampled", "STATUS UNAVAILABLE");
  for (const id of ["hosts", "attention", "journal"]) {
    $(id).replaceChildren(element(id === "hosts" ? "p" : "li", "empty", message));
  }
}

export function showError(error) {
  const target = $("error");
  target.hidden = !error;
  target.textContent = error ? `Status unavailable: ${error.message}. The page will retry.` : "";
}

export function showConnection(connected) {
  text("connection", connected ? "● LIVE" : "◷ RECONNECTING");
}
