import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  agentState,
  cooldown,
  hostNote,
  hostState,
  issueRows,
  liveTitle,
  peerDashboardUrl,
  peerMachine,
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
  const capturedAt = Date.parse(captured.hosts.items[1].cooldown_until) - 41 * 60_000;
  const summary = tiles([captured.board], captured.hosts, captured.attention, capturedAt);
  assert.equal(summary.workers, "1 / 1 ISSUES");
  assert.equal(summary.workerNote, "0 working, 0 blocked, 0 idle.");
  assert.equal(summary.attention, "0 OPEN");
  assert.equal(summary.attentionNote, "Nothing waiting on you.");
  assert.equal(summary.hosts, "2 / 4");
  assert.equal(summary.plans, "3 / 4");
  assert.equal(summary.planNote, "Across 1 worker plans.");
  const withPeer = tiles(
    [captured.board, captured.peer_board],
    captured.hosts,
    captured.attention,
    capturedAt,
  );
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
  assert.equal(hostNote(host, now), "usage limit · back in 41:00");
  assert.equal(hostState({ ...host, paused: true }, now).label, "‖ PAUSED");
  assert.equal(hostNote({ ...host, paused: true }, now), "No new launches.");
  assert.equal(hostState(captured.hosts.items[3], now).label, "‖ PAUSED");
  assert.equal(hostState(captured.hosts.items[0], now).label, "● IN ROTATION");
  assert.equal(hostNote(captured.hosts.items[0], now), "Ready for new launches.");
});

test("live titles describe the host's actual title semantics", () => {
  assert.equal(liveTitle("claude", "local-658"), "session local-658");
  assert.equal(liveTitle("muse", "worktree"), "folder worktree");
  assert.equal(liveTitle("codex", "Fix dashboard | worktree"), "title Fix dashboard | worktree");
});

test("peer lookup distinguishes a reachable peer from local and unavailable machines", () => {
  assert.equal(peerMachine(captured.fleet, "build-mini")?.state, "ok");
  assert.equal(peerMachine(captured.fleet, "local"), undefined);
  const unavailable = structuredClone(captured.fleet);
  unavailable.machines[1].state = "unreachable";
  assert.equal(peerMachine(unavailable, "build-mini")?.state, "unreachable");
});

test("peer dashboard links accept only a root HTTP URL without embedded credentials", () => {
  const valid = captured.fleet.machines[1].dashboard_url;
  const credential = new URL(valid);
  credential.username = "user";
  credential.password = "password";
  assert.equal(peerDashboardUrl(valid), valid);
  for (const invalid of [
    ["javascript", ":alert(1)"].join(""),
    credential.href,
    new URL("/path", valid).href,
    new URL("/?token=secret", valid).href,
    new URL("/#fragment", valid).href,
    "not a URL",
  ]) {
    assert.equal(peerDashboardUrl(invalid), null, invalid);
  }
});
