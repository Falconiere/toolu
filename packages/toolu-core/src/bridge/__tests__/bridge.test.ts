import { expect, test } from "bun:test";
import { parseBridgeRequest } from "../bridge.ts";

test("parseBridgeRequest defaults args", () => {
  expect(parseBridgeRequest({ op: "ping" }).args).toEqual({});
});
