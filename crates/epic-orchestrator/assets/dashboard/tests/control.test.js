import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  actionBody,
  actionRequest,
  actionResult,
  attentionAction,
  controlDisabled,
  counter,
  drawerActions,
  journalLine,
  liveAgent,
  refusalCode,
  sendDisabled,
} from "../js/logic/control.js";

const captured = JSON.parse(
  readFileSync(new URL("../../../../../fixtures/epic/dashboard/status.json", import.meta.url)),
);
const pick = (list) => list.map(({ action, disabled }) => `${action}:${disabled}`);
const running = { ...captured.board.issues[0], stage: "running", agent_status: "working" };

test("engine attention kinds map to answer, release or ack; unknown kinds ack", () => {
  for (const kind of ["needs-human", "stuck", "help", "contract-changed"]) {
    assert.equal(attentionAction(kind, "k-1"), "answer", kind);
  }
  assert.equal(attentionAction("coupling-hold", "k-1"), "release");
  for (const kind of [
    "failed",
    "stall",
    "blocked",
    "babysit",
    "pr-closed",
    "dependency-cycle",
    "herdr-error",
    "merge-ready",
    "relaunch-limit",
    "unblocked",
    "judgment",
    "brand-new-kind",
  ]) {
    assert.equal(attentionAction(kind, "k-1"), "ack", kind);
  }
  assert.equal(attentionAction("needs-human", ""), "ack");
  assert.equal(attentionAction("coupling-hold", null), "ack");
});

test("a live agent is launched and not done", () => {
  assert.equal(liveAgent(running), true);
  assert.equal(liveAgent({ ...running, stage: "awaiting_merge", agent_status: "idle" }), true);
  assert.equal(liveAgent({ ...running, stage: "" }), false);
  assert.equal(liveAgent({ ...running, stage: "merged" }), false);
  assert.equal(liveAgent({ ...running, agent_status: "done" }), false);
  assert.equal(liveAgent({ ...running, agent_status: "unknown" }, "working"), true);
  assert.equal(liveAgent(null), false);
});

test("buttons disable while locked or pending; send also while empty", () => {
  assert.equal(controlDisabled({ locked: false, pending: false }), false);
  assert.equal(controlDisabled({ locked: true, pending: false }), true);
  assert.equal(controlDisabled({ locked: false, pending: true }), true);
  assert.equal(sendDisabled({ locked: false, pending: false, text: "ship it" }), false);
  assert.equal(sendDisabled({ locked: false, pending: false, text: " \n\t" }), true);
  assert.equal(sendDisabled({ locked: false, pending: false, text: "" }), true);
  assert.equal(sendDisabled({ locked: true, pending: false, text: "ship it" }), true);
  assert.equal(sendDisabled({ locked: false, pending: true, text: "ship it" }), true);
});

test("drawer actions: answer needs a live agent, release needs a hold, ack open attention", () => {
  const open = { kind: "needs-human", key: running.key };
  const base = { locked: false, pending: false };
  assert.deepEqual(pick(drawerActions({ ...base, issue: running })), ["answer:false"]);
  assert.deepEqual(pick(drawerActions({ ...base, issue: running, attention: open })), [
    "answer:false",
    "ack:false",
  ]);
  // A coupling hold exists only before launch: RELEASE HOLD stays enabled without an agent.
  const held = { ...running, stage: "", hold: { with: "k-2" } };
  assert.deepEqual(pick(drawerActions({ ...base, issue: held })), ["answer:true", "release:false"]);
  assert.deepEqual(pick(drawerActions({ ...base, locked: true, issue: held, attention: open })), [
    "answer:true",
    "release:true",
    "ack:true",
  ]);
  assert.deepEqual(pick(drawerActions({ ...base, pending: true, issue: running })), [
    "answer:true",
  ]);
  assert.equal(drawerActions({ ...base, issue: running })[0].label, "ANSWER →");
});

test("requests carry the token only in the Authorization header", () => {
  const token = "abc+/=TOKEN";
  const { path, init } = actionRequest("host-resume", { host: "codex" }, token);
  assert.equal(path, "/api/v1/actions/host-resume");
  assert.ok(!path.includes(token));
  assert.equal(init.method, "POST");
  assert.equal(init.headers.Authorization, `Bearer ${token}`);
  assert.equal(init.headers["Content-Type"], "application/json");
  assert.equal(init.body, '{"host":"codex"}');
  assert.ok(!init.body.includes(token));
});

test("journal lines match the engine's action records", () => {
  assert.equal(journalLine("pause", {}), "pause all by=web");
  assert.equal(journalLine("resume", { epic: "o-r-649" }), "resume o-r-649 by=web");
  assert.equal(journalLine("host-resume", { host: "codex" }), "host-resumed codex by=web");
  assert.equal(journalLine("host-pause", { host: "claude" }), "host-paused claude by=web");
  assert.equal(journalLine("release", { key: "k-1" }), "release k-1 by=web");
  assert.equal(journalLine("ack", { kind: "babysit" }), "ack babysit by=web");
});

test("results read sent, delivered, uncertain, refused and failed", () => {
  assert.deepEqual(actionResult("pause", {}, 200, { ok: true }), {
    kind: "sent",
    label: "✓ SENT pause all by=web",
    text: "The effect lands through the journal stream.",
  });
  const answer = { key: "k-1", text: "yes" };
  assert.equal(actionResult("answer", answer, 200, { ok: true }).label, "✓ DELIVERED");
  assert.equal(
    actionResult("answer", answer, 200, { ok: true }).text,
    "Attention marked answered.",
  );
  const uncertain = { ok: true, delivered: "uncertain", note: "worker prompt may not have…" };
  assert.equal(actionResult("answer", answer, 200, uncertain).label, "▲ UNCERTAIN");
  // The text `herdr_calls::result_of` builds for a herdr error reply.
  const blocked = {
    schema: 1,
    error: "worker prompt not delivered: herdr agent.prompt: agent_blocked: agent is blocked",
  };
  const refused = actionResult("answer", answer, 409, blocked);
  assert.equal(refused.label, "● REFUSED · agent_blocked");
  assert.equal(refused.text, "The engine refused the answer. Attach from a terminal.");
  assert.equal(refused.detail, blocked.error);
  assert.equal(
    actionResult("answer", answer, 409, { schema: 1, error: "no live agent for k-1" }).label,
    "● REFUSED · no live agent for k-1",
  );
  assert.equal(
    actionResult("release", { key: "k-1" }, 409, { schema: 1, error: "k-1 is not held" }).text,
    "k-1 is not held",
  );
  assert.equal(
    actionResult("pause", {}, 503, { schema: 1, error: "engine_busy" }).label,
    "● FAILED · engine_busy",
  );
  assert.equal(actionResult("pause", {}, 0, null).label, "● FAILED · network");
  assert.equal(actionResult("pause", {}, 502, null).label, "● FAILED · HTTP 502");
});

test("refusal codes come from herdr's error code, else the whole text", () => {
  assert.equal(
    refusalCode("worker prompt not delivered: herdr agent.prompt: timeout: x"),
    "timeout",
  );
  assert.equal(refusalCode("k-1 is not held"), "k-1 is not held");
  assert.equal(refusalCode(undefined), "");
});

test("the answer counter counts characters against 4000", () => {
  assert.equal(counter(""), "0 / 4000");
  assert.equal(counter("é\n"), "2 / 4000");
});

test("buttons send the op body the action API validates", () => {
  assert.deepEqual(actionBody("pause", {}), {});
  assert.deepEqual(actionBody("resume", { epic: "o-r-649" }), { epic: "o-r-649" });
  assert.deepEqual(actionBody("host-resume", { host: "codex" }), { host: "codex" });
  assert.deepEqual(actionBody("ack", { key: "k-1", kind: "failed" }), { key: "k-1" });
  assert.deepEqual(actionBody("ack", { key: "", kind: "babysit" }), { kind: "babysit" });
  assert.deepEqual(actionBody("release", { key: "k-1" }), { key: "k-1" });
});
