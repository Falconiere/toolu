import { hostNote, hostState, tiles } from "../logic/status.js";
import { renderEpic, renderLive, showBoardMessage } from "./board.js";
import { $, element, mark, text } from "./dom.js";

// Rebuilt rows take their old action slots back, so a button mid-click survives a render.
const slotsBy = (target, selector, id) =>
  new Map([...target.querySelectorAll(selector)].map((slot) => [id(slot.dataset), slot]));
const attentionSlotId = ({ kind, key, state }) => `${kind} ${key} ${state}`;

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

function renderTiles(docs, live, sampledAt, now) {
  const elapsed = Math.max(0, now - sampledAt);
  const attention = {
    items: (docs.attention.items ?? []).map((item) => ({
      ...item,
      age_ms: (item.age_ms ?? 0) + elapsed,
    })),
  };
  const totals = tiles(docs.boards, docs.hosts, attention, now, live);
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

const HOST_NAMES = {
  claude: "Claude Code",
  codex: "Codex",
  cursor: "Cursor Agent",
  muse: "Muse Code",
  grok: "Grok Build",
};

function renderHosts(hosts, now) {
  const target = $("hosts");
  const slots = slotsBy(target, ".host-control-slot", (data) => data.host);
  target.replaceChildren();
  for (const host of hosts.items ?? []) {
    const row = element("div", "host-row");
    const head = element("div", "host-head");
    head.append(
      element("strong", "", HOST_NAMES[host.host] || host.host),
      mark(hostState(host, now)),
    );
    const slot = slots.get(host.host) ?? element("div", "host-control-slot");
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
  const slots = slotsBy(target, ".attention-action-slot", attentionSlotId);
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
    const data = { kind: item.kind || "", key: item.key || "", state: item.state || "open" };
    const slot = slots.get(attentionSlotId(data)) ?? element("span", "attention-action-slot");
    Object.assign(slot.dataset, data);
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

// `view` picks the epic board to draw; every other section is drawn whatever is selected.
export function renderDashboard(docs, live, peer, sampledAt, view, now = Date.now()) {
  const attention = renderTiles(docs, live, sampledAt, now);
  const board = view.kind === "epic" && docs.boards.find((item) => item.key === view.key);
  renderEpic(board || null, live, attention, peer);
  renderLive(docs.boards, live, attention, peer);
  renderHosts(docs.hosts, now);
  renderAttention(attention, docs.boards);
  renderJournal(docs.events, peer);
  text(
    "sampled",
    `${peer ? "PEER · POLLED 5S" : "LIVE · POLLED 3S"} · SAMPLED ${new Date(sampledAt).toLocaleTimeString([], { hour12: false })}`,
  );
}

export function renderTick(docs, live, sampledAt, now = Date.now()) {
  const attention = renderTiles(docs, live, sampledAt, now);
  renderHosts(docs.hosts, now);
  renderAttention(attention, docs.boards);
}

export function renderUnavailable(message) {
  showBoardMessage(message);
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
