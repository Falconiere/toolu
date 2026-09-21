import { expect, test } from "bun:test";
import { parseTooluConfig } from "../config.ts";

test("parseTooluConfig requires version 1", () => {
  expect(parseTooluConfig({ version: 1 }).version).toBe(1);
  expect(() => parseTooluConfig({ version: 2 })).toThrow();
});
