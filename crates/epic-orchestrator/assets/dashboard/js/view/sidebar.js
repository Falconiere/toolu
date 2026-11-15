import { sidebarEntries } from "../logic/board.js";
import { $, changed, element, redraw } from "./dom.js";

const VIEWS = ["overview", "epic", "hosts", "journal"];
const viewId = (view) => view.key ?? view.kind;

function entryButton(entry, active) {
  const button = element(
    "button",
    entry.complete ? "sidebar-entry sidebar-complete" : "sidebar-entry",
  );
  button.type = "button";
  button.dataset.kind = entry.view.kind;
  button.dataset.view = viewId(entry.view);
  if (viewId(entry.view) === viewId(active)) button.setAttribute("aria-current", "page");
  button.append(
    element("span", "sidebar-label", entry.label),
    element("span", "sidebar-badge", entry.badge),
  );
  if (entry.title) button.append(element("span", "sidebar-title", entry.title));
  if (entry.meta) button.append(element("span", "sidebar-meta", entry.meta));
  return button;
}

// `active` is the selected view, or the epic of the open worker.
export function renderSidebar(docs, active, now = Date.now()) {
  const nav = $("sidebar");
  const entries = sidebarEntries(docs, now);
  if (!changed(nav, [entries, active])) return;
  const nodes = entries.map((entry) => entryButton(entry, active));
  const first = entries.findIndex((entry) => entry.view.kind === "epic");
  if (first >= 0) nodes.splice(first, 0, element("p", "sidebar-heading", "EPICS"));
  redraw(
    nav,
    (node) => node.dataset.view,
    () => nav.replaceChildren(...nodes),
  );
}

// Exactly one view section is visible; the worker page covers it while open.
export function showView(view) {
  for (const kind of VIEWS) $(`view-${kind}`).hidden = kind !== view.kind;
}

export function bindSidebar(select) {
  $("sidebar").addEventListener("click", (event) => {
    const button = event.target.closest?.("button[data-view]");
    if (!button) return;
    const { kind, view } = button.dataset;
    select(kind === "epic" ? { kind, key: view } : { kind });
  });
}
