import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { originOf } from "../checkouts.ts";

test("originOf reads the real checkout and rejects a non-repository", () => {
  const root = resolve(import.meta.dir, "../../../..");
  expect(originOf(root)).toBe("falconiere/toolu");
  expect(originOf("/tmp")).toBeNull();
});
