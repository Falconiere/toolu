import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  agentState,
  cooldown,
  hostState,
  issueRows,
  phaseLine,
  tiles,
  waveLine,
} from "../js/logic/status.js";

const captured = JSON.parse(
  readFileSync(new URL("../../../../../fixtures/epic/dashboard/status.json", import.meta.url)),
);

test("agent labels follow issue lifecycle before last agent observation", () => {
  const issue = structuredClone(captured.board.issues[0]);
  assert.equal(agentState(issue).label, "? UNKNOWN");
  issue.agent_status = "working";
  assert.equal(agentState(issue).label, "● WORKING");
  issue.agent_status = "blocked";
  assert.equal(agentState(issue).label, "▲ BLOCKED");
  issue.agent_status = "idle";
  assert.equal(agentState(issue).label, "/ IDLE");
  issue.agent_status = "done";
  assert.equal(agentState(issue).label, "✓ DONE");
  issue.agent_status = "unknown";
  assert.equal(agentState(issue, "working").label, "● WORKING");
  issue.stage = "";
  assert.equal(agentState(issue, "working").label, "○ QUEUED");
  issue.stage = "merged";
  assert.equal(agentState(issue, "working").label, "✓ DONE");
});

test("wave line and rows use the engine's stable wave and graph title", () => {
  assert.equal(waveLine(captured.board), "WAVE 1 OF 1 · 1 LIVE · 0 QUEUED");
  const complete = { ...captured.board, current_wave: null, live: 0 };
  assert.equal(waveLine(complete), "WAVES COMPLETE · 0 LIVE · 0 QUEUED");
  assert.deepEqual(
    issueRows(captured.board).map(({ key, title, wave }) => [key, title, wave]),
    [["local-658", "Fleet", 1]],
  );
  const issue = { ...captured.board.issues[0], hold: { with: "local-657" } };
  assert.equal(phaseLine(issue, 1), "HELD · with local-657");
  assert.equal(phaseLine({ ...issue, hold: null, stage: "" }, 2), "QUEUED · WAVE 2");
});

test("tiles aggregate the captured board, plan, hosts, and empty attention", () => {
  const summary = tiles([captured.board], captured.hosts, captured.attention);
  assert.equal(summary.workers, "1 / 1 ISSUES");
  assert.equal(summary.workerNote, "0 working, 0 blocked, 0 idle.");
  assert.equal(summary.attention, "0 OPEN");
  assert.equal(summary.attentionNote, "Nothing waiting on you.");
  assert.equal(summary.hosts, "4 / 4");
  assert.equal(summary.plans, "1 / 4");
  assert.equal(summary.planNote, "Across 1 worker plans.");
  const withPeer = tiles([captured.board, captured.peer_board], captured.hosts, captured.attention);
  assert.equal(withPeer.workers, "2 / 2 ISSUES");
});

test("cooldown counts down, expires, and yields to a paused host", () => {
  const now = Date.parse("2026-10-27T12:00:00Z");
  const until = "2026-10-27T12:41:00Z";
  assert.equal(cooldown(until, now), "41:00");
  assert.equal(cooldown(until, now + 1000), "40:59");
  assert.equal(cooldown(until, now + 2_460_000), null);
  const host = { ...captured.hosts.items[0], cooldown_until: until };
  assert.equal(hostState(host, now).label, "◷ COOLING");
  assert.equal(hostState({ ...host, paused: true }, now).label, "‖ PAUSED");
  assert.equal(hostState(captured.hosts.items[0], now).label, "● IN ROTATION");
});
