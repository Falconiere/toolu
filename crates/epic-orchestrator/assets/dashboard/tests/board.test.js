import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  liveRows,
  resolveView,
  shortKey,
  sidebarEntries,
  waveGroups,
  waveLabel,
} from "../js/logic/board.js";

const captured = JSON.parse(
  readFileSync(new URL("../../../../../fixtures/epic/dashboard/status.json", import.meta.url)),
);
const hash = (number) => ["#", number].join("");
// Codex is cooling for 41 minutes and Muse is paused in the capture.
const capturedAt = Date.parse(captured.hosts.items[1].cooldown_until) - 41 * 60_000;
const [epic] = captured.epics.items;
const [worker] = captured.board.issues;

// The captured board grown to three waves: wave 1 merged and closed before launch,
// wave 2 the captured running worker beside a never-launched issue, wave 3 queued.
function threeWaves() {
  const board = structuredClone(captured.board);
  const node = (key, wave) => ({ ...board.graph.issues[0], key, wave, title: key });
  board.graph.issues = [node("a-1", 1), node("b-2", 1), node(worker.key, 2), node("c-3", 2)];
  board.graph.issues.push(node("d-4", 3));
  board.graph.waves = [
    { wave: 1, issues: ["a-1", "b-2"] },
    { wave: 2, issues: [worker.key, "c-3"] },
    { wave: 3, issues: ["d-4"] },
  ];
  board.issues.push({ ...worker, key: "a-1", stage: "merged" });
  Object.assign(board, { current_wave: 2, waves_total: 3, queued: 2 });
  return board;
}

test("sidebar lists overview, each epic, hosts and journal with their marks", () => {
  assert.deepEqual(sidebarEntries(captured, capturedAt), [
    { view: { kind: "overview" }, label: "Overview", badge: "" },
    {
      view: { kind: "epic", key: epic.key },
      label: `EPIC ${hash(649)}`,
      title: "Dashboard redesign",
      meta: "‖ PAUSED · WAVE 1/1 · 1 LIVE",
      badge: "",
      complete: false,
    },
    { view: { kind: "hosts" }, label: "Hosts", badge: "3/5" },
    { view: { kind: "journal" }, label: "Journal", badge: "" },
  ]);
});

test("sidebar counts open attention and sorts complete epics last", () => {
  const done = { ...epic, key: "o-r-12", title: "", paused: false, current_wave: null, live: 0 };
  const busy = { ...epic, key: "o-r-30", paused: false, attention_count: 2, queued: 3 };
  const docs = {
    ...captured,
    epics: { items: [done, busy] },
    attention: { items: [{ key: worker.key, state: "open" }, { state: "acknowledged" }] },
  };
  const entries = sidebarEntries(docs, capturedAt);
  assert.equal(entries[0].badge, "▲ 1");
  assert.deepEqual(
    entries
      .slice(1, 3)
      .map(({ label, title, meta, badge, complete }) => [label, title, meta, badge, complete]),
    [
      [`EPIC ${hash(30)}`, "Dashboard redesign", "WAVE 1/1 · 1 LIVE · 3 QUEUED", "▲ 2", false],
      [`EPIC ${hash(12)}`, "o-r-12", "✓ COMPLETE", "", true],
    ],
  );
  assert.deepEqual(
    sidebarEntries({}).map((entry) => [entry.label, entry.badge]),
    [
      ["Overview", ""],
      ["Hosts", ""],
      ["Journal", ""],
    ],
  );
});

test("a view of an epic the documents do not hold resolves to the overview", () => {
  const view = { kind: "epic", key: epic.key };
  assert.equal(resolveView(view, captured.epics.items), view);
  assert.deepEqual(resolveView({ kind: "epic", key: "gone-1" }, captured.epics.items), {
    kind: "overview",
  });
  assert.deepEqual(resolveView(view, undefined), { kind: "overview" });
  assert.deepEqual(resolveView({ kind: "hosts" }, []), { kind: "hosts" });
});

test("wave groups follow the graph waves and open only the current one", () => {
  const [only] = waveGroups(captured.board);
  assert.deepEqual(
    [only.wave, only.current, only.open, only.live, only.total, only.rows[0].key],
    [1, true, true, 1, 1, worker.key],
  );
  assert.equal(waveLabel(only), "WAVE 1 · CURRENT · 1 LIVE");
  const groups = waveGroups(threeWaves());
  assert.deepEqual(
    groups.map(({ wave, current, open, done, live, queued, total }) => [
      wave,
      current,
      open,
      done,
      live,
      queued,
      total,
    ]),
    [
      [1, false, false, 2, 0, 0, 2],
      [2, true, true, 0, 1, 1, 2],
      [3, false, false, 0, 0, 1, 1],
    ],
  );
  assert.deepEqual(groups.map(waveLabel), [
    "WAVE 1 · 2 DONE",
    "WAVE 2 · CURRENT · 1 LIVE · 1 QUEUED",
    "WAVE 3 · 1 QUEUED",
  ]);
  // A never-launched issue in a wave the epic has passed was closed before launch.
  assert.deepEqual(
    groups[0].rows.map((row) => [row.key, row.closed]),
    [
      ["a-1", false],
      ["b-2", true],
    ],
  );
});

test("a running worker whose agent ended a turn is live, not done", () => {
  // Codex, Cursor, Muse and Grok report `done` after every turn.
  const board = structuredClone(captured.board);
  board.issues[0].agent_status = "done";
  const [group] = waveGroups(board);
  assert.deepEqual([group.live, group.done], [1, 0]);
  assert.equal(waveLabel(group), "WAVE 1 · CURRENT · 1 LIVE");
});

test("a board without waves is one open group and a complete board opens none", () => {
  const loose = structuredClone(captured.board);
  loose.graph.waves = [];
  loose.graph.issues[0].wave = null;
  const [group, ...rest] = waveGroups(loose);
  assert.deepEqual([group.wave, group.open, group.total, rest.length], [null, true, 1, 0]);
  assert.equal(waveLabel(group), "UNSCHEDULED · 1 ISSUE");
  const complete = structuredClone(captured.board);
  complete.issues[0].stage = "merged";
  Object.assign(complete, { current_wave: null, live: 0 });
  const groups = waveGroups(complete);
  assert.deepEqual(
    groups.map((item) => [item.open, waveLabel(item)]),
    [[false, "WAVE 1 · 1 DONE"]],
  );
  assert.deepEqual(waveGroups({ key: "empty" }), []);
});

test("live rows are the running and merging workers of every board", () => {
  const board = threeWaves();
  board.issues.push({ ...worker, key: "d-4", stage: "awaiting_merge" });
  const rows = liveRows([board, captured.peer_board]);
  assert.deepEqual(
    rows.map(({ board: owner, row }) => [owner.key, row.key, row.title]),
    [
      [board.key, worker.key, worker.key],
      [board.key, "d-4", "d-4"],
      [captured.peer_board.key, captured.peer_board.issues[0].key, "Fleet"],
    ],
  );
  assert.deepEqual(liveRows([]), []);
  assert.deepEqual(liveRows(undefined), []);
});

test("short keys keep the issue number and leave other keys whole", () => {
  assert.equal(shortKey("falconiere-toolu-870"), hash(870));
  assert.equal(shortKey("PROJ-12"), hash(12));
  assert.equal(shortKey("drawer-blocked"), "drawer-blocked");
  assert.equal(shortKey(""), "");
});
