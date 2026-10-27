import { actionRequest } from "./logic/control.js";

async function get(path) {
  const response = await fetch(path, { cache: "no-store" });
  if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
  return response.json();
}

const root = (peer) => (peer ? `/api/v1/fleet/peers/${encodeURIComponent(peer)}` : "/api/v1");

export const readFleet = () => get("/api/v1/fleet");
export const readLive = () => get("/api/v1/agents/live");

async function readBoards(base, epics) {
  const batch = await Promise.all(
    epics.slice(0, 4).map((epic) => get(`${base}/epics/${encodeURIComponent(epic.key)}`)),
  );
  return epics.length > 4 ? batch.concat(await readBoards(base, epics.slice(4))) : batch;
}

export async function readDashboard(peer) {
  const base = root(peer);
  const [overview, epics, hosts, attention] = await Promise.all(
    ["overview", "epics", "hosts", "attention"].map((name) => get(`${base}/${name}`)),
  );
  const boards = await readBoards(base, epics.items ?? []);
  const events = peer
    ? null
    : await get(`/api/v1/events?since=${Math.max(0, (overview.journal_seq ?? 0) - 7)}&limit=7`);
  return { overview, epics, boards, hosts, attention, events };
}

export const readIssue = (peer, key) => get(`${root(peer)}/issues/${encodeURIComponent(key)}`);

// The screen route is local only and always needs the bearer token.
async function privateGet(path, token) {
  const response = await fetch(path, {
    cache: "no-store",
    headers: { Authorization: `Bearer ${token}` },
  });
  return { status: response.status, body: await response.json().catch(() => null) };
}

// Actions are local only; a status, not an exception, reports an HTTP refusal.
export async function postAction(op, body, token) {
  const { path, init } = actionRequest(op, body, token);
  try {
    const response = await fetch(path, init);
    return { status: response.status, body: await response.json().catch(() => null) };
  } catch {
    return { status: 0, body: null };
  }
}

export function readScreen(key, token) {
  const path = `/api/v1/issues/${encodeURIComponent(key)}/screen`;
  return privateGet(path, token);
}

export function readLogs(key, token) {
  const path = `/api/v1/issues/${encodeURIComponent(key)}/logs`;
  return privateGet(path, token);
}

export function readLog(key, id, offset, token) {
  const path = `/api/v1/issues/${encodeURIComponent(key)}/logs/${encodeURIComponent(id)}${offset == null ? "" : `?offset=${offset}`}`;
  return privateGet(path, token);
}
