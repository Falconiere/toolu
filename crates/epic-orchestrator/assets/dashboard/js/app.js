import { readDashboard, readFleet, readLive } from "./api.js";
import { peerMachine } from "./logic/status.js";
import { renderPeer } from "./view/peer.js";
import {
  renderDashboard,
  renderMachines,
  renderTick,
  renderUnavailable,
  showConnection,
  showError,
} from "./view/dashboard.js";

const state = { peer: null, fleet: null, docs: null, live: null, sampledAt: 0, request: 0 };
let stream;
let refreshTimer;

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
    renderDashboard(docs, state.peer ? null : state.live, state.peer, state.sampledAt);
    showError(null);
    showConnection(true);
    startStream();
  } catch (error) {
    if (request !== state.request) return;
    if (!state.docs) renderUnavailable("Status is unavailable. Retrying…");
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
      renderDashboard(state.docs, state.live, null, state.sampledAt);
    }
  } catch {
    state.live = null;
  }
}

function select(peer) {
  if (state.peer === peer) return;
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
  renderUnavailable("Loading status…");
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
  if (!document.hidden && state.docs) renderTick(state.docs, state.sampledAt);
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
