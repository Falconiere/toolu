import { expect, test } from "bun:test";
import { lifecycleSupport } from "../table.ts";

test("session/start supported; permission/evaluate deferred for #204", () => {
  expect(lifecycleSupport("session/start")).toBe("supported");
  expect(lifecycleSupport("permission/evaluate")).toBe("deferred");
});
