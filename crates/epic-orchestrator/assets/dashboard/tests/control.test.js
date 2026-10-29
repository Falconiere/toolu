import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  actionBody,
  actionRequest,
  actionResult,
  answerBody,
  attentionAction,
  controlDisabled,
  counter,
  drawerActions,
  journalLine,
  keyAttention,
  liveAgent,
  mintSubmissionId,
  refusalCode,
  rememberSubmission,
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
  // The engine keeps acknowledged and answered items listed with their state.
  assert.equal(attentionAction("blocked", "k-1", "acknowledged"), null);
  assert.equal(attentionAction("needs-human", "k-1", "answered"), null);
  assert.equal(attentionAction("needs-human", "k-1", "open"), "answer");
  assert.equal(attentionAction("coupling-hold", null), "ack");
});

test("a key's open attention item wins over its handled ones", () => {
  const acked = { key: "k-1", kind: "stall", state: "acknowledged", note: "old" };
  const open = { key: "k-1", kind: "needs-human", state: "open", note: "which API?" };
  const other = { key: "k-2", kind: "failed", state: "open" };
  assert.equal(keyAttention({ items: [acked, other, open] }, "k-1"), open);
  assert.equal(keyAttention({ items: [acked, other] }, "k-1"), acked);
  assert.equal(keyAttention({ items: [other] }, "k-1"), undefined);
  assert.equal(keyAttention(null, "k-1"), undefined);
});

test("a live agent is launched and its issue not finished, as the engine decides", () => {
  assert.equal(liveAgent(running), true);
  assert.equal(liveAgent({ ...running, stage: "awaiting_merge", agent_status: "idle" }), true);
  // herdr reports `done` at the end of every Codex turn and of a Claude Code turn.
  assert.equal(liveAgent({ ...running, agent_status: "done" }), true);
  assert.equal(liveAgent({ ...running, stage: "needs-human" }), true);
  assert.equal(liveAgent({ ...running, stage: "" }), false);
  for (const stage of ["merged", "cleaning", "cleanup-incomplete", "abandoned"]) {
    assert.equal(liveAgent({ ...running, stage }), false, stage);
  }
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
  const acked = { ...open, state: "acknowledged" };
  assert.deepEqual(pick(drawerActions({ ...base, issue: running, attention: acked })), [
    "answer:false",
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
  assert.equal(journalLine("release", { key: "k-1" }, "k-2"), "release k-1 k-2 by=web");
  assert.equal(journalLine("ack", { key: "k-1" }), "acknowledged k-1 by=web");
  assert.equal(journalLine("ack", { kind: "babysit" }), "acknowledged babysit by=web");
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
    actionResult("pause", {}, 200, {
      ok: true,
      delivered: "uncertain",
      note: "engine reply not confirmed before deadline",
    }).label,
    "▲ UNCERTAIN",
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

test("answer bodies mint a submission_id the engine can dedupe", () => {
  const first = mintSubmissionId();
  const second = mintSubmissionId();
  assert.ok(first.length >= 1 && first.length <= 128);
  assert.match(first, /^[A-Za-z0-9._-]+$/);
  assert.notEqual(first, second);
  assert.deepEqual(answerBody("k-1", "take B", "sub-1"), {
    key: "k-1",
    text: "take B",
    submission_id: "sub-1",
  });
  const minted = answerBody("k-1", "take B");
  assert.equal(minted.key, "k-1");
  assert.equal(minted.text, "take B");
  assert.match(minted.submission_id, /^[A-Za-z0-9._-]+$/);
});

test("rememberSubmission reuses an id for A after A→B→A in one dialog", () => {
  const map = new Map();
  let n = 0;
  const mint = () => `id-${++n}`;
  const a1 = rememberSubmission(map, "k-1", "A", mint);
  const b = rememberSubmission(map, "k-1", "B", mint);
  const a2 = rememberSubmission(map, "k-1", "A", mint);
  assert.equal(a1, "id-1");
  assert.equal(b, "id-2");
  assert.equal(a2, "id-1");
});
