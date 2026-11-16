import assert from "node:assert/strict";
import test from "node:test";

import {
  fragmentIssue,
  fragmentView,
  takeToken,
  withIssue,
  withView,
} from "../js/logic/fragment.js";

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
  // `toolu epic token url` prints the token unencoded; a base64 `+` is not a space.
  assert.deepEqual(takeToken("#token=a+b/c=&issue=k"), { token: "a+b/c=", hash: "#issue=k" });
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

test("the selected view is read from the fragment and an epic wins over a panel", () => {
  assert.deepEqual(fragmentView("#epic=o-r-9"), { kind: "epic", key: "o-r-9" });
  assert.deepEqual(fragmentView("#view=hosts"), { kind: "hosts" });
  assert.deepEqual(fragmentView("#view=journal&issue=k"), { kind: "journal" });
  assert.deepEqual(fragmentView("#view=hosts&epic=o-r-9"), { kind: "epic", key: "o-r-9" });
  for (const hash of ["", undefined, "#view=nope", "#view=", "#epic=", "#issue=k"]) {
    assert.deepEqual(fragmentView(hash), { kind: "overview" }, String(hash));
  }
});

test("setting a view keeps the issue and the token and clears the other view key", () => {
  const epic = { kind: "epic", key: "o-r-9" };
  assert.equal(withView("#issue=k", epic), "#issue=k&epic=o-r-9");
  assert.equal(withView("#epic=o-r-9&issue=k", { kind: "hosts" }), "#issue=k&view=hosts");
  assert.equal(withView("#view=hosts", epic), "#epic=o-r-9");
  assert.equal(withView("#epic=o-r-9&view=journal", { kind: "overview" }), "");
  // A base64 token survives the rewrite: `+` is still a plus when it is read back.
  assert.deepEqual(takeToken(withView("#token=a+b/c=&issue=k", epic)), {
    token: "a+b/c=",
    hash: "#issue=k&epic=o-r-9",
  });
});
