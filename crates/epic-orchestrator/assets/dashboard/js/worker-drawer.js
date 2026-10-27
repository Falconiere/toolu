import { readIssue, readLog, readLogs, readScreen } from "./api.js";
import { controlState, readToken, rotated } from "./control.js";
import { keyAttention } from "./logic/control.js";
import { wantsScreen } from "./logic/drawer.js";
import { appendPage, wantsLogs } from "./logic/logs.js";
import { fragmentIssue, withIssue } from "./logic/fragment.js";
import { findIssue, peerMachine } from "./logic/status.js";
import { renderDrawerActions } from "./view/controls.js";
import { bindDrawer, hideDrawer, renderDrawer, revealDrawer } from "./view/drawer.js";

const POLL_MS = 2000;
const EMPTY = {
  doc: null,
  docFailed: false,
  screen: null,
  screenToken: null,
  tab: "screen",
  logItems: null,
  logId: null,
  logRaw: "",
  logOffset: null,
  logError: null,
  logUnavailable: null,
  logTruncated: false,
};
const drawer = { key: null, opener: null, ...EMPTY, timer: undefined, seq: 0 };
let inflight = -1;
let logTimer;
let logRequest = 0;
let current = () => ({});

const replaceHash = (hash) =>
  history.replaceState(null, "", `${location.pathname}${location.search}${hash}`);
const setHash = (key) => replaceHash(withIssue(location.hash, key));

const find = (key) => findIssue(current().docs?.boards, key);

function render() {
  const found = find(drawer.key);
  if (!found) return;
  const { docs, live, peer, fleet, sampledAt } = current();
  const attention = keyAttention(docs.attention, drawer.key);
  const liveItem = peer ? null : (live?.items ?? []).find((item) => item.key === drawer.key);
  renderDrawer({
    key: drawer.key,
    ...found,
    doc: drawer.doc,
    docFailed: drawer.docFailed,
    screen: drawer.screen,
    tab: drawer.tab,
    logs: {
      items: drawer.logItems,
      id: drawer.logId,
      raw: drawer.logRaw,
      error: drawer.logError,
      unavailable: drawer.logUnavailable,
      truncated: drawer.logTruncated,
    },
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
    const token = readToken();
    // 403 means the engine has no token configured; ask again only with another token.
    const disabled = drawer.screen?.status === 403 && drawer.screenToken === token;
    // The board row decides the screen read, so both reads run at once.
    const wanted =
      drawer.tab === "screen" &&
      wantsScreen({ peer, issue: { ...drawer.doc, ...find(key)?.row }, token });
    const [doc, read] = await Promise.all([
      readIssue(peer, key).catch(() => null),
      !disabled && wanted
        ? readScreen(key, token).catch((error) => ({ error: error.message }))
        : null,
    ]);
    const screen = disabled ? drawer.screen : read;
    if (screen?.status === 401) rotated(token);
    if (seq !== drawer.seq) return;
    drawer.docFailed = !doc;
    drawer.doc = doc ?? drawer.doc;
    drawer.screen = screen;
    drawer.screenToken = token;
    render();
  } finally {
    if (inflight === seq) inflight = -1;
  }
}

const logContext = () => ({
  key: drawer.key,
  token: readToken(),
  peer: current().peer,
  tab: drawer.tab,
  visible: document.visibilityState === "visible",
});

function stopLogs() {
  clearTimeout(logTimer);
  logRequest += 1;
}

function scheduleLogs() {
  clearTimeout(logTimer);
  if (wantsLogs(logContext())) logTimer = setTimeout(logTick, 1000);
}

async function logTick() {
  if (!wantsLogs(logContext())) return;
  const request = ++logRequest;
  const { key } = drawer;
  const token = readToken();
  try {
    const listed = await readLogs(key, token).catch((error) => ({ error: error.message }));
    if (request !== logRequest) return;
    if (listed.status === 401) rotated(token);
    if (listed.status !== 200 || !Array.isArray(listed.body?.items)) {
      drawer.logError = listed.body?.reason || listed.body?.error || listed.error || "read failed";
      render();
      return;
    }
    drawer.logError = null;
    drawer.logItems = listed.body.items;
    const id = drawer.logItems.some((item) => item.id === drawer.logId)
      ? drawer.logId
      : (drawer.logItems[0]?.id ?? null);
    if (id !== drawer.logId) {
      drawer.logId = id;
      drawer.logRaw = "";
      drawer.logOffset = null;
      drawer.logUnavailable = null;
      drawer.logTruncated = false;
    }
    if (id) {
      const page = await readLog(key, id, drawer.logOffset, token).catch((error) => ({
        error: error.message,
      }));
      if (request !== logRequest) return;
      if (page.status === 401) rotated(token);
      if (page.status === 200 && typeof page.body?.text === "string") {
        const next = appendPage({ raw: drawer.logRaw, nextOffset: drawer.logOffset }, page.body);
        drawer.logRaw = next.raw;
        drawer.logOffset = next.nextOffset;
        drawer.logTruncated ||= next.truncated;
        drawer.logUnavailable = null;
      } else {
        drawer.logUnavailable =
          page.body?.reason || page.body?.error || page.error || "read failed";
      }
    }
    render();
  } finally {
    if (request === logRequest) scheduleLogs();
  }
}

function setTab(tab) {
  if (drawer.tab === tab) return;
  stopLogs();
  drawer.tab = tab;
  render();
  if (tab === "logs") logTick();
  else tick();
}

function selectLog(id) {
  if (drawer.logId === id) return;
  stopLogs();
  drawer.logId = id;
  drawer.logRaw = "";
  drawer.logOffset = null;
  drawer.logUnavailable = null;
  drawer.logTruncated = false;
  render();
  logTick();
}

export function openDrawer(key, opener) {
  if (!find(key)) return false;
  if (key !== drawer.key) {
    stopLogs();
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
  stopLogs();
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
  bindDrawer(() => closeDrawer(true), setTab, selectLog);
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
    else if (drawer.tab === "logs") logTick();
  });
  document.addEventListener("visibilitychange", tick);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") logTick();
    else stopLogs();
  });
}
