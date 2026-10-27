import assert from "node:assert/strict";
import test from "node:test";

import { appendPage, stripAnsi, wantsLogs } from "../js/logic/logs.js";

test("log polling is limited to a visible local Logs tab with a token", () => {
  const ready = { key: "worker", token: "secret", peer: null, tab: "logs", visible: true };
  assert.equal(wantsLogs(ready), true);
  for (const change of [
    { key: null },
    { token: null },
    { peer: "box" },
    { tab: "screen" },
    { visible: false },
  ]) {
    assert.equal(wantsLogs({ ...ready, ...change }), false);
  }
});

test("a follow page appends at the cursor and a truncated page resets stale text", () => {
  const first = appendPage(
    { raw: "", nextOffset: null },
    {
      text: "first\n",
      offset: 10,
      next_offset: 16,
      truncated: false,
      state: "running",
    },
  );
  assert.equal(first.raw, "first\n");
  assert.equal(first.nextOffset, 16);
  const next = appendPage(first, {
    text: "last\n",
    offset: 16,
    next_offset: 21,
    truncated: false,
    state: "done",
  });
  assert.equal(next.raw, "first\nlast\n");
  assert.equal(next.state, "done");
  const trimmed = appendPage(next, {
    text: "tail",
    offset: 100,
    next_offset: 104,
    truncated: true,
    state: "done",
  });
  assert.equal(trimmed.raw, "tail");
  assert.equal(trimmed.truncated, true);
  const long = appendPage(
    { raw: "a".repeat(1_048_576), nextOffset: 0 },
    {
      text: "tail",
      offset: 0,
      next_offset: 4,
      truncated: false,
      state: "running",
    },
  );
  assert.equal(long.raw.length, 1_048_576);
  assert.ok(long.raw.endsWith("tail"));
});

test("trimming a log skips a terminal title sequence cut in half", () => {
  const cutInsideTitle = appendPage(
    { raw: "", nextOffset: null },
    {
      text: "a".repeat(100) + "\u001b]" + "private title".repeat(100_000) + "\u0007tail",
      offset: 0,
      next_offset: 1,
      truncated: false,
    },
  );
  assert.ok(!stripAnsi(cutInsideTitle.raw).includes("private title"));
  assert.equal(cutInsideTitle.raw, "tail");
});

test("terminal color and title escape sequences never reach visible log text", () => {
  assert.equal(stripAnsi("\u001b[31mred\u001b[0m plain"), "red plain");
  assert.equal(stripAnsi("before\u001b]0;private title\u0007after"), "beforeafter");
  assert.equal(stripAnsi("text\u001b[31"), "text");
  assert.equal(stripAnsi("a\u001bPprivate\u001b\\b\u001b(B"), "ab");
});
