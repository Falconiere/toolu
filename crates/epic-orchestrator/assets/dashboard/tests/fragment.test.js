import assert from "node:assert/strict";
import test from "node:test";

import { fragmentIssue, takeToken, withIssue } from "../js/logic/fragment.js";

const token = "0f".repeat(32);

test("the token leaves the fragment and the remaining keys stay", () => {
  assert.deepEqual(takeToken(`#token=${token}&issue=toolu-655`), {
    token,
    hash: "#issue=toolu-655",
  });
  assert.deepEqual(takeToken(`#token=${token}`), { token, hash: "" });
  assert.deepEqual(takeToken("#issue=toolu-655"), { token: null, hash: "#issue=toolu-655" });
  assert.deepEqual(takeToken(""), { token: null, hash: "" });
  assert.deepEqual(takeToken("#token="), { token: null, hash: "" });
});

test("issue deep links read, set and clear only the issue key", () => {
  assert.equal(fragmentIssue("#issue=toolu-655"), "toolu-655");
  assert.equal(fragmentIssue("#issue=a%2Eb"), "a.b");
  assert.equal(fragmentIssue("#other=1"), null);
  assert.equal(fragmentIssue(undefined), null);
  assert.equal(withIssue("", "toolu-655"), "#issue=toolu-655");
  assert.equal(withIssue("#issue=old", "toolu-655"), "#issue=toolu-655");
  assert.equal(withIssue("#issue=toolu-655", null), "");
  assert.equal(withIssue("#x=1&issue=toolu-655", null), "#x=1");
});
