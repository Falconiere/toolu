import { expect, test } from "bun:test";
import { parseId } from "../parse-id.ts";

test("parseId accepts a string", () => {
  expect(parseId("abc")).toBe("abc");
});
