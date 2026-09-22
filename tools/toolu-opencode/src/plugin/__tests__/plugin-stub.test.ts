import { expect, test } from "bun:test";
import { classifyStub, detectHost } from "../plugin-stub.ts";

test("classifyStub uses core policy", () => {
  expect(classifyStub("port-native")).toBe("port-native");
});

test("plugin-stub re-exports detectHost", () => {
  expect(detectHost({ env: { TOOLU_HOST_OVERRIDE: "opencode" } })).toBe("opencode");
});
