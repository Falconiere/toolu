import assert from "node:assert/strict";
import test from "node:test";

import { formatCount } from "../js/logic/format.js";

test("formats a count", () => {
  assert.equal(formatCount(2), "2");
});

test("Node globals are available to tests", () => {
  assert.equal(typeof process, "object");
});
