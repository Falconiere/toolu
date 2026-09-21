import { expect, test } from "bun:test";
import { parseClassification } from "../policy.ts";

test("parseClassification accepts shell-out", () => {
  expect(parseClassification("shell-out")).toBe("shell-out");
});
