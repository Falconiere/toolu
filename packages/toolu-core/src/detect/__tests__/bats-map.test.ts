/**
 * Every detect bats case has a bun-test home (#254 AC-9). The test reads the
 * `@test` names from the bats files and fails on an unmapped name, a mapped home
 * that does not exist, a map entry no bats name uses, or a harvested-fixture
 * home holding no `detect.bats` case for its function.
 */
import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { z } from "zod";
import { BATS_FILES, BATS_MAP, batsPrefix } from "./bats-map.ts";

const REPO = resolve(import.meta.dir, "../../../../..");

const names = BATS_FILES.flatMap((file) =>
  [...readFileSync(join(REPO, file), "utf8").matchAll(/^@test "(.+)" \{$/gm)].map(
    (m) => m[1] ?? "",
  ),
);

const harvested = z
  .object({
    cases: z.array(z.object({ fn: z.string(), suites: z.array(z.string()) }).passthrough()),
  })
  .parse(JSON.parse(readFileSync(join(REPO, "tooling/fixtures/shell/bats-parity.json"), "utf8")))
  .cases.filter((c) => c.suites.includes("toolu/hooks/lib/__tests__/detect.bats"));

test("both bats files are read", () => {
  expect(names.length).toBeGreaterThanOrEqual(155);
});

test("every bats case maps to a home, and every map entry is used", () => {
  const unmapped = names.filter((name) => BATS_MAP[batsPrefix(name)] === undefined);
  expect(unmapped).toEqual([]);
  const used = new Set(names.map(batsPrefix));
  expect(Object.keys(BATS_MAP).filter((key) => !used.has(key))).toEqual([]);
});

test.each(Object.entries(BATS_MAP))(
  "%s: the home exists and holds its cases",
  (key, { home, fixtureFn }) => {
    expect(existsSync(join(REPO, home))).toBe(true);
    if (fixtureFn === undefined) {
      expect(readFileSync(join(REPO, home), "utf8")).toMatch(/\btest(\.concurrent)?(\.each)?\(/);
      return;
    }
    const count = harvested.filter((c) => c.fn === fixtureFn).length;
    const bats = names.filter((name) => batsPrefix(name) === key).length;
    expect({ key, count: count > 0 }).toEqual({ key, count: true });
    expect(bats).toBeGreaterThan(0);
  },
);
