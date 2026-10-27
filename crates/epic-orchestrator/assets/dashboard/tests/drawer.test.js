import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  NO_PLAN,
  PLAN_UNREADABLE,
  attachCommand,
  attentionLine,
  clock,
  epicLine,
  nextStep,
  phaseTimeline,
  planLine,
  prUrl,
  screenNotice,
  stepMark,
  wantsScreen,
} from "../js/logic/drawer.js";

const captured = JSON.parse(
  readFileSync(new URL("../../../../../fixtures/epic/dashboard/status.json", import.meta.url)),
);
const issue = captured.issue;
const queued = { key: "toolu-655", open_blockers: ["Falconiere/toolu#651", "o/r#654"] };

test("ledger step statuses map to glyph and word, red and stale included", () => {
  assert.deepEqual(
    ["green", "running", "pending", "red", "stale", "skipped", "constructor"].map(
      (status) => stepMark(status).label,
    ),
    ["✓ GREEN", "● IN PROGRESS", "○ TODO", "✕ FAILING", "↺ STALE", "? UNKNOWN", "? UNKNOWN"],
  );
  assert.equal(stepMark("running").kind, "running");
  for (const step of issue.plan.steps) assert.notEqual(stepMark(step.status).kind, "unknown");
});

test("plan summary and next step come from the captured ledger", () => {
  const { plan } = issue;
  assert.equal(planLine(plan), `${plan.summary.green}/${plan.summary.total} GREEN`);
  assert.equal(nextStep(plan), plan.steps.find((step) => step.id === plan.next).title);
  assert.equal(nextStep({ next: "gone", steps: [] }), "gone");
  assert.equal(nextStep(null), null);
  assert.equal(planLine(null), "0/0 GREEN");
  assert.match(PLAN_UNREADABLE.text, /^The ledger failed to parse\. Phase history shown instead\.$/);
  assert.match(NO_PLAN, /^No plan yet\./);
});

test("notices follow peer, queue, finish, token and screen replies in order", () => {
  const token = "t";
  const notice = (context) => screenNotice({ issue, token, ...context })?.label ?? null;
  assert.equal(notice({ peer: "build-mini" }), "◇ OPEN DASHBOARD TO WATCH OR CONTROL");
  assert.equal(
    screenNotice({ issue: queued, token }).text,
    "Queued until #651, #654 merges. The screen appears once a host claims it.",
  );
  assert.equal(
    screenNotice({ issue: { open_blockers: [] } }).text,
    "Queued. The screen appears once a host claims it.",
  );
  assert.equal(
    screenNotice({ issue: { stage: "merged", phase: "babysit" }, token }).text,
    "Finished · last phase BABYSIT.",
  );
  assert.equal(notice({ token: null }), "○ CONTROL LOCKED");
  assert.equal(notice({ screen: captured.screen_locked }), "○ CONTROL LOCKED");
  assert.equal(notice({ screen: captured.screen_disabled }), "○ CONTROL LOCKED");
  assert.equal(notice({ screen: captured.screen }), "? LIVE VIEW UNAVAILABLE");
  assert.equal(notice({ screen: { error: "network" } }), "? LIVE VIEW UNAVAILABLE");
  assert.equal(notice({ screen: { status: 503, body: { error: "herdr_busy" } } }), "? LIVE VIEW UNAVAILABLE");
  assert.equal(notice({ screen: { status: 200, body: { state: "not_running" } } }), "/ AGENT NOT RUNNING");
  assert.equal(notice({ screen: { status: 200, body: { text: "$ ls", lines: 1 } } }), null);
  assert.equal(notice({ screen: null }), null);
});

test("screen reads are wanted only for a local, launched, unfinished worker with a token", () => {
  assert.equal(wantsScreen({ issue, token: "t" }), true);
  assert.equal(wantsScreen({ issue, token: null }), false);
  assert.equal(wantsScreen({ issue, token: "t", peer: "build-mini" }), false);
  assert.equal(wantsScreen({ issue: queued, token: "t" }), false);
  assert.equal(wantsScreen({ issue: { stage: "abandoned" }, token: "t" }), false);
});

test("attach command is the served herdr agent attach string for a blocked agent only", () => {
  const blocked = (attach) => ({ status: 200, body: { agent_status: "blocked", attach } });
  assert.equal(attachCommand(blocked("herdr agent attach toolu-651")), "herdr agent attach toolu-651");
  assert.equal(
    attachCommand(blocked("herdr --session work agent attach 'a b'")),
    "herdr --session work agent attach 'a b'",
  );
  assert.equal(attachCommand(blocked("  ")), null);
  assert.equal(attachCommand(blocked(undefined)), null);
  assert.equal(
    attachCommand({ status: 200, body: { agent_status: "working", attach: "herdr agent attach x" } }),
    null,
  );
  assert.equal(attachCommand(captured.screen), null);
});

test("phase timeline lists history with the last phase current, or QUEUED", () => {
  const rows = phaseTimeline(issue.phase_history, issue);
  assert.equal(rows.length, issue.phase_history.length);
  assert.equal(rows.at(-1).phase, issue.phase_history.at(-1).phase.toUpperCase());
  assert.equal(rows.at(-1).current, true);
  assert.match(rows[0].at, /^\d\d:\d\d$/);
  assert.deepEqual(phaseTimeline([], queued), [{ phase: "QUEUED", at: "—", current: true }]);
  assert.deepEqual(phaseTimeline(undefined, issue), []);
});

test("pull request links derive from the issue URL and refuse other shapes", () => {
  const url = captured.board.url;
  assert.equal(prUrl(url, 693), url.replace(/\/issues\/\d+$/, "/pull/693"));
  assert.equal(prUrl(url, null), null);
  assert.equal(prUrl(url, 0), null);
  assert.equal(prUrl("javascript:alert(1)", 3), null);
  assert.equal(prUrl("https://github.com/o/r/pull/3", 3), null);
  assert.equal(prUrl("not a url", 3), null);
});

test("attention, epic and clock lines", () => {
  const item = { kind: "needs-human", age_ms: 61_000 };
  assert.equal(attentionLine(item, 2_000, 1_000), "▲ NEEDS-HUMAN · OPEN 1m");
  assert.equal(attentionLine({}, 0, 0), "▲ UNKNOWN · OPEN 0m");
  assert.equal(epicLine("toolu-655", "falconiere-toolu-649"), "toolu-655 · EPIC #649");
  assert.match(clock("2026-10-27T21:13:54Z", true), /^\d\d:\d\d:\d\d$/);
  assert.equal(clock(null), "—");
  assert.equal(clock("garbage"), "—");
});
