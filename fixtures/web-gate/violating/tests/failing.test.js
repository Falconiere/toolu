import assert from "node:assert/strict";
import test from "node:test";

test("failing fixture", () => {
  assert.equal(1, 2);
});
