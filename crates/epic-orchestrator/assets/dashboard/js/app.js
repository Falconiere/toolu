import { readDashboard, readFleet, readLive } from "./api.js";
import { resolveView } from "./logic/board.js";
import { fragmentIssue, fragmentView, withView } from "./logic/fragment.js";
import { findIssue, peerMachine } from "./logic/status.js";
import { bindBoard } from "./view/board.js";
import { renderPeer } from "./view/peer.js";
import { bindSidebar, renderSidebar, showView } from "./view/sidebar.js";
import {
  renderDashboard,
  renderMachines,
  renderTick,
  renderUnavailable,
  showConnection,
  showError,
} from "./view/dashboard.js";
import { closeAnswer } from "./answer-dialog.js";
import { controlState, initControl } from "./control.js";
import { focusedControl, renderControls, restoreControl } from "./view/controls.js";
import { closeDrawer, initDrawer, syncDrawer } from "./worker-drawer.js";

const state = { peer: null, fleet: null, docs: null, live: null, sampledAt: 0, request: 0 };
let stream;
let refreshTimer;

// The selected view comes from the URL fragment on every paint; nothing else stores it.
const selected = () => resolveView(fragmentView(location.hash), state.docs?.epics?.items);

function renderNav() {
  const view = selected();
  // An open worker belongs to its epic, whatever view it was opened from.
  const epic = findIssue(state.docs?.boards, fragmentIssue(location.hash))?.board.key;
  renderSidebar(state.docs, epic ? { kind: "epic", key: epic } : view);
  showView(view);
}

// Renders rebuild the rows that hold action buttons; keep keyboard focus on the same control.
function paint(draw = () => {}) {
  const focus = focusedControl();
  draw();
  renderNav();
  renderControls({ ...controlState(), docs: state.docs, now: Date.now() });
  restoreControl(focus);
}

// Always paints: the sidebar and controls are drawn even before the first documents load.
function repaint() {
  const live = state.peer ? null : state.live;
  paint(() => {
    if (state.docs) renderDashboard(state.docs, live, state.peer, state.sampledAt, selected());
  });
}

function navigate(view) {
  closeDrawer();
  closeAnswer();
  history.replaceState(
    null,
    "",
    `${location.pathname}${location.search}${withView(location.hash, view)}`,
  );
  repaint();
  scrollTo({ top: 0 });
}

initControl(
  () => state,
  () => {
    paint();
    syncDrawer();
  },
);
initDrawer(() => state, repaint);
bindSidebar(navigate);
bindBoard();
window.addEventListener("hashchange", repaint);

function stopStream() {
  stream?.close();
  stream = undefined;
}

function startStream() {
  if (stream || state.peer || document.hidden || !state.docs) return;
  stream = new EventSource(`/api/v1/events/stream?since=${state.docs.overview.journal_seq ?? 0}`);
  stream.addEventListener("journal", queueRefresh);
  stream.addEventListener("reset", queueRefresh);
  stream.addEventListener("error", () => {
    stopStream();
    showConnection(false);
  });
}

function queueRefresh() {
  clearTimeout(refreshTimer);
  refreshTimer = setTimeout(refresh, 150);
}

async function refresh() {
  if (document.hidden) return;
  const request = ++state.request;
  const peer = state.peer && peerMachine(state.fleet, state.peer);
  if (state.peer && peer?.state !== "ok") {
    if (!state.docs) renderUnavailable(`Peer ${peer?.state ?? "unavailable"}. Retrying…`);
    showError(new Error(`peer ${peer?.state ?? "unavailable"}`));
    showConnection(false);
    return;
  }
  try {
    const docs = await readDashboard(state.peer);
    if (request !== state.request) return;
    state.docs = docs;
    state.sampledAt = Date.now();
    repaint();
    syncDrawer();
    showError(null);
    showConnection(true);
    startStream();
  } catch (error) {
    if (request !== state.request) return;
    if (!state.docs) paint(() => renderUnavailable("Status is unavailable. Retrying…"));
    showError(error);
    showConnection(false);
  }
}

async function refreshFleet() {
  if (document.hidden) return;
  try {
    const fleet = await readFleet();
    state.fleet = fleet;
    renderMachines(fleet, state.peer, select);
    renderPeer(peerMachine(fleet, state.peer));
  } catch (error) {
    showError(error);
  }
}

async function sampleLive() {
  if (document.hidden || state.peer) return;
  try {
    const live = await readLive();
    if (state.peer || document.hidden) return;
    state.live = live;
    if (state.docs) {
      repaint();
      syncDrawer();
    }
  } catch {
    state.live = null;
  }
}

function select(peer) {
  if (state.peer === peer) return;
  closeDrawer();
  closeAnswer();
  state.peer = peer;
  state.docs = null;
  state.live = null;
  ++state.request;
  stopStream();
  clearTimeout(refreshTimer);
  if (state.fleet) {
    renderMachines(state.fleet, peer, select);
    renderPeer(peerMachine(state.fleet, peer));
  }
  paint(() => renderUnavailable("Loading status…"));
  refresh();
  if (!peer) sampleLive();
}

document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    stopStream();
    clearTimeout(refreshTimer);
    return;
  }
  refreshFleet();
  refresh();
  sampleLive();
});

setInterval(() => {
  if (!document.hidden && state.docs) {
    paint(() => renderTick(state.docs, state.live, state.sampledAt));
  }
}, 1000);
setInterval(() => {
  if (!document.hidden) {
    refreshFleet();
    if (state.peer || !stream) refresh();
  }
}, 5000);
setInterval(sampleLive, 3000);

refreshFleet();
refresh();
sampleLive();
