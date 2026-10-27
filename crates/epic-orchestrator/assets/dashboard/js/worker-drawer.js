import { readIssue, readScreen } from "./api.js";
import { wantsScreen } from "./logic/drawer.js";
import { fragmentIssue, withIssue } from "./logic/fragment.js";
import { issueRows, peerMachine } from "./logic/status.js";
import { bindDrawer, hideDrawer, renderDrawer } from "./view/drawer.js";

const TOKEN = "toolu.token";
const POLL_MS = 2000;
const drawer = { key: null, opener: null, doc: null, screen: null, timer: undefined, seq: 0 };
let inflight = -1;
let current = () => ({});

function storage(action) {
  try {
    return action(sessionStorage);
  } catch {
    return null;
  }
}
export const readToken = () => storage((store) => store.getItem(TOKEN));
export const saveToken = (token) => storage((store) => store.setItem(TOKEN, token));
const dropToken = () => storage((store) => store.removeItem(TOKEN));

function setHash(key) {
  const hash = withIssue(location.hash, key);
  history.replaceState(null, "", `${location.pathname}${location.search}${hash}`);
}

function find(key) {
  for (const board of current().docs?.boards ?? []) {
    const row = issueRows(board).find((issue) => issue.key === key);
    if (row) return { board, row };
  }
  return null;
}

function render() {
  const found = find(drawer.key);
  if (!found) return;
  const { docs, live, peer, fleet, sampledAt } = current();
  renderDrawer({
    key: drawer.key,
    ...found,
    doc: drawer.doc,
    screen: drawer.screen,
    attention: (docs.attention?.items ?? []).find((item) => item.key === drawer.key),
    live: peer ? null : (live?.items ?? []).find((item) => item.key === drawer.key),
    token: readToken(),
    peer,
    machine: peerMachine(fleet, peer),
    now: Date.now(),
    sampledAt,
  });
}

// Reads run only while the drawer is open and the tab is visible.
async function tick() {
  const { key, seq } = drawer;
  if (!key || inflight === seq || document.visibilityState !== "visible") return;
  inflight = seq;
  const { peer } = current();
  try {
    const doc = await readIssue(peer, key).catch(() => drawer.doc);
    const token = readToken();
    let screen = null;
    if (wantsScreen({ peer, issue: { ...find(key)?.row, ...doc }, token })) {
      screen = await readScreen(key, token).catch((error) => ({ error: error.message }));
    }
    if (screen?.status === 401) dropToken();
    if (seq !== drawer.seq) return;
    drawer.doc = doc;
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
    drawer.doc = null;
    drawer.screen = null;
  }
  drawer.key = key;
  drawer.opener = opener;
  setHash(key);
  clearInterval(drawer.timer);
  drawer.timer = setInterval(tick, POLL_MS);
  render();
  tick();
  return true;
}

export function closeDrawer(restoreFocus = false) {
  const { key, opener } = drawer;
  if (!key) return;
  clearInterval(drawer.timer);
  Object.assign(drawer, { key: null, opener: null, doc: null, screen: null, timer: undefined });
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
    if (event.key !== "Escape" || !drawer.key) return;
    event.preventDefault();
    closeDrawer(true);
  });
  window.addEventListener("hashchange", () => {
    const key = fragmentIssue(location.hash);
    if (!key) closeDrawer();
    else if (key !== drawer.key && !openDrawer(key, null)) setHash(null);
  });
  document.addEventListener("visibilitychange", tick);
}
