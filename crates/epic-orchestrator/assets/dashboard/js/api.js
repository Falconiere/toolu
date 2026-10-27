async function get(path) {
  const response = await fetch(path, { cache: "no-store" });
  if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
  return response.json();
}

export const readFleet = () => get("/api/v1/fleet");
export const readLive = () => get("/api/v1/agents/live");

async function readBoards(base, epics) {
  const batch = await Promise.all(
    epics.slice(0, 4).map((epic) => get(`${base}/epics/${encodeURIComponent(epic.key)}`)),
  );
  return epics.length > 4 ? batch.concat(await readBoards(base, epics.slice(4))) : batch;
}

export async function readDashboard(peer) {
  const base = peer ? `/api/v1/fleet/peers/${encodeURIComponent(peer)}` : "/api/v1";
  const [overview, epics, hosts, attention] = await Promise.all(
    ["overview", "epics", "hosts", "attention"].map((name) => get(`${base}/${name}`)),
  );
  const boards = await readBoards(base, epics.items ?? []);
  const events = peer
    ? null
    : await get(`/api/v1/events?since=${Math.max(0, (overview.journal_seq ?? 0) - 7)}&limit=7`);
  return { overview, epics, boards, hosts, attention, events };
}
