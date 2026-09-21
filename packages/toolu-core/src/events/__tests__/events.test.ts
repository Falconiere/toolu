import { expect, test } from "bun:test";
import { parseNormalizedEvent } from "../events.ts";

test("parseNormalizedEvent requires type", () => {
  expect(parseNormalizedEvent({ type: "x", payload: null }).type).toBe("x");
  expect(() => parseNormalizedEvent({ payload: 1 })).toThrow();
});
