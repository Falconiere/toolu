import { readIssue, readScreen } from "./api.js";
import { wantsScreen } from "./logic/drawer.js";
import { fragmentIssue, takeToken, withIssue } from "./logic/fragment.js";
import { peerMachine } from "./logic/status.js";
import { bindDrawer, hideDrawer, renderDrawer, revealDrawer } from "./view/drawer.js";

const TOKEN = "toolu.token";
const POLL_MS = 2000;
const EMPTY = { doc: null, docFailed: false, screen: null };
const drawer = { key: null, opener: null, ...EMPTY, timer: undefined, seq: 0 };
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
const saveToken = (token) => storage((store) => store.setItem(TOKEN, token));
const dropToken = () => storage((store) => store.removeItem(TOKEN));

const replaceHash = (hash) =>
  history.replaceState(null, "", `${location.pathname}${location.search}${hash}`);
const setHash = (key) => replaceHash(withIssue(location.hash, key));

// A token URL opened in this tab, on load or later as a hash change, moves to sessionStorage.
function storeToken() {
  const fragment = takeToken(location.hash);
  if (!fragment.token) return;
  saveToken(fragment.token);
  replaceHash(fragment.hash);
}

function find(key) {
  for (const board of current().docs?.boards ?? []) {
    const node = (board.graph?.issues ?? []).find((issue) => issue.key === key);
    if (node) {
      return { board, row: { ...node, ...board.issues?.find((issue) => issue.key === key) } };
    }
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
    docFailed: drawer.docFailed,
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
    const doc = await readIssue(peer, key).catch(() => null);
    const token = readToken();
    // 403 means the engine has no token configured; asking again cannot succeed.
    let screen = drawer.screen?.status === 403 ? drawer.screen : null;
    if (!screen && wantsScreen({ peer, issue: { ...doc, ...find(key)?.row }, token })) {
      screen = await readScreen(key, token).catch((error) => ({ error: error.message }));
    }
    if (screen?.status === 401 && readToken() === token) dropToken();
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
  storeToken();
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
    storeToken();
    const key = fragmentIssue(location.hash);
    if (!key) closeDrawer();
    // Before boards load, syncDrawer opens the deep link instead.
    else if (key !== drawer.key && current().docs && !openDrawer(key, null)) setHash(null);
  });
  document.addEventListener("visibilitychange", tick);
}
