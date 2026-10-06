import { expect, test } from "bun:test";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { parseTooluConfig, TooluConfigSchema } from "../config.ts";

const REPO = resolve(import.meta.dir, "../../../../..");
const FIXTURES = join(REPO, "fixtures/config");
const EXAMPLE = join(REPO, "plugins/toolu/settings/toolu.config.example.json");

function readJson(path: string): unknown {
  return JSON.parse(readFileSync(path, "utf8"));
}

test.concurrent("parseTooluConfig accepts version 1 and rejects other versions", () => {
  expect(parseTooluConfig({ version: 1 }).version).toBe(1);
  expect(() => parseTooluConfig({ version: 2 })).toThrow();
  expect(() => parseTooluConfig({ version: "1" })).toThrow();
});

test.concurrent("a missing version is version 1", () => {
  expect(parseTooluConfig({}).version).toBeUndefined();
});

test.concurrent("parseTooluConfig rejects unknown top-level keys", () => {
  expect(() => parseTooluConfig({ version: 1, extra: true })).toThrow();
});

test.concurrent("parseTooluConfig accepts gates protectedFiles block", () => {
  const cfg = parseTooluConfig({ version: 1, gates: { protectedFiles: { mode: "block" } } });
  expect(cfg.gates?.protectedFiles?.mode).toBe("block");
});

test.concurrent("parseTooluConfig rejects invalid gate mode", () => {
  expect(() =>
    parseTooluConfig({ version: 1, gates: { protectedFiles: { mode: "maybe" } } }),
  ).toThrow();
});

test.concurrent("the shipped example config parses, prBabysit included", () => {
  const cfg = parseTooluConfig(readJson(EXAMPLE));
  expect(cfg.prBabysit?.dispatch).toBe("herdr");
});

test.concurrent("this repository's own comemory config parses", () => {
  expect(parseTooluConfig(readJson(join(FIXTURES, "repo-own-comemory.json"))).comemory).toEqual({
    setup_done: true,
  });
});

test.concurrent("every docs/config.md example fixture parses", () => {
  const docs = readdirSync(FIXTURES).filter((name) => name.startsWith("docs-"));
  expect(docs.length).toBeGreaterThanOrEqual(6);
  for (const name of docs) {
    expect(TooluConfigSchema.safeParse(readJson(join(FIXTURES, name))).success).toBe(true);
  }
});

test.concurrent("prBabysit rejects keys pr-babysit does not read", () => {
  expect(() => parseTooluConfig({ prBabysit: { dispatch: "herdr", colour: "red" } })).toThrow();
});
