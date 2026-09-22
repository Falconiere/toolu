import { expect, test } from "bun:test";
import { lifecycleSupport } from "../table.ts";

test("session/start and permission/evaluate supported after #204", () => {
  expect(lifecycleSupport("session/start")).toBe("supported");
  expect(lifecycleSupport("permission/evaluate")).toBe("supported");
});
