import { readIssue, readScreen } from "./api.js";
import { controlState, readToken, rotated } from "./control.js";
import { wantsScreen } from "./logic/drawer.js";
import { fragmentIssue, withIssue } from "./logic/fragment.js";
import { findIssue, peerMachine } from "./logic/status.js";
import { renderDrawerActions } from "./view/controls.js";
import { bindDrawer, hideDrawer, renderDrawer, revealDrawer } from "./view/drawer.js";

const POLL_MS = 2000;
const EMPTY = { doc: null, docFailed: false, screen: null };
const drawer = { key: null, opener: null, ...EMPTY, timer: undefined, seq: 0 };
let inflight = -1;
let current = () => ({});

const replaceHash = (hash) =>
  history.replaceState(null, "", `${location.pathname}${location.search}${hash}`);
const setHash = (key) => replaceHash(withIssue(location.hash, key));

const find = (key) => findIssue(current().docs?.boards, key);

function render() {
  const found = find(drawer.key);
  if (!found) return;
  const { docs, live, peer, fleet, sampledAt } = current();
  const attention = (docs.attention?.items ?? []).find((item) => item.key === drawer.key);
  const liveItem = peer ? null : (live?.items ?? []).find((item) => item.key === drawer.key);
  renderDrawer({
    key: drawer.key,
    ...found,
    doc: drawer.doc,
    docFailed: drawer.docFailed,
    screen: drawer.screen,
    attention,
    live: liveItem,
    token: readToken(),
    peer,
    machine: peerMachine(fleet, peer),
    now: Date.now(),
    sampledAt,
  });
  renderDrawerActions({
    ...controlState(),
    issue: { ...drawer.doc, ...found.row },
    attention,
  });
}

// Reads run only while the drawer is open and the tab is visible.
async function tick() {
  const { key, seq } = drawer;
  if (!key || inflight === seq || document.visibilityState !== "visible") return;
  inflight = seq;
  const { peer } = current();
  try {
    const doc = await readIssue(peer, key).catch(() => null);
    const token = readToken();
    // 403 means the engine has no token configured; asking again cannot succeed.
    let screen = drawer.screen?.status === 403 ? drawer.screen : null;
    if (!screen && wantsScreen({ peer, issue: { ...doc, ...find(key)?.row }, token })) {
      screen = await readScreen(key, token).catch((error) => ({ error: error.message }));
    }
    if (screen?.status === 401) rotated(token);
    if (seq !== drawer.seq) return;
    drawer.docFailed = !doc;
    drawer.doc = doc ?? drawer.doc;
    drawer.screen = screen;
    render();
  } finally {
    if (inflight === seq) inflight = -1;
  }
}

export function openDrawer(key, opener) {
  if (!find(key)) return false;
  if (key !== drawer.key) {
    drawer.seq += 1;
    Object.assign(drawer, EMPTY);
  }
  drawer.key = key;
  drawer.opener = opener;
  setHash(key);
  clearInterval(drawer.timer);
  drawer.timer = setInterval(tick, POLL_MS);
  render();
  if (opener) revealDrawer();
  tick();
  return true;
}

export function closeDrawer(restoreFocus = false) {
  const { key, opener } = drawer;
  if (!key) return;
  clearInterval(drawer.timer);
  Object.assign(drawer, { key: null, opener: null, ...EMPTY, timer: undefined });
  drawer.seq += 1;
  hideDrawer();
  setHash(null);
  if (!restoreFocus) return;
  const rows = [...document.querySelectorAll("#board button[data-issue]")];
  (opener?.isConnected ? opener : rows.find((row) => row.dataset.issue === key))?.focus();
}

// After each dashboard render: refresh the open drawer, or open the deep-linked one.
export function syncDrawer() {
  if (!current().docs) return;
  if (drawer.key) {
    if (find(drawer.key)) render();
    else closeDrawer();
    return;
  }
  const key = fragmentIssue(location.hash);
  if (key && !openDrawer(key, null)) setHash(null);
}

export function initDrawer(state) {
  current = state;
  bindDrawer(() => closeDrawer(true));
  document.addEventListener("click", (event) => {
    const button = event.target.closest?.("button[data-issue]");
    if (button) openDrawer(button.dataset.issue, button);
  });
  document.addEventListener("keydown", (event) => {
    // Esc in the answer dialog cancels the dialog, not the drawer behind it.
    if (event.key !== "Escape" || !drawer.key || document.querySelector("dialog[open]")) return;
    event.preventDefault();
    closeDrawer(true);
  });
  window.addEventListener("hashchange", () => {
    const key = fragmentIssue(location.hash);
    if (!key) closeDrawer();
    // Before boards load, syncDrawer opens the deep link instead.
    else if (key !== drawer.key && current().docs && !openDrawer(key, null)) setHash(null);
  });
  document.addEventListener("visibilitychange", tick);
}
