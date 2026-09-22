import { expect, test } from "bun:test";
import { parseTooluConfig } from "../config.ts";

test("parseTooluConfig requires version 1", () => {
  expect(parseTooluConfig({ version: 1 }).version).toBe(1);
  expect(() => parseTooluConfig({ version: 2 })).toThrow();
});

test("parseTooluConfig rejects unknown top-level keys", () => {
  expect(() => parseTooluConfig({ version: 1, extra: true })).toThrow();
});

test("parseTooluConfig accepts gates protectedFiles block", () => {
  const cfg = parseTooluConfig({
    version: 1,
    gates: { protectedFiles: { mode: "block" } },
  });
  expect(cfg.gates?.protectedFiles?.mode).toBe("block");
});

test("parseTooluConfig rejects invalid gate mode", () => {
  expect(() =>
    parseTooluConfig({
      version: 1,
      gates: { protectedFiles: { mode: "maybe" } },
    }),
  ).toThrow();
});
