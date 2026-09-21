import { expect, test } from "bun:test";
import { classifyStub } from "../plugin-stub.ts";

test("classifyStub uses core policy", () => {
  expect(classifyStub("port-native")).toBe("port-native");
});
