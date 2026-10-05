import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { contractPaths } from "../../opencode-host/results.ts";
import { ProbeResultsSchema, readJson } from "../../opencode-host/schema.ts";
import { ROOT } from "../../opencode-host/scenarios-entry.ts";
import {
  catalogNames,
  coverage,
  inSequence,
  selectChecks,
  type AcceptanceCheck,
} from "../checks.ts";
import { acceptanceChecks } from "../families.ts";
import { LIVE_TEST_FILES } from "../live-tests.ts";

// The real registry against the real catalog (#362 AC-3): no plugin may lack an actual-host check.

const committed = readJson(contractPaths().results, ProbeResultsSchema);
const checks = acceptanceChecks(committed);
const catalog = catalogNames();

test.concurrent("the catalog has the 12 plugins the acceptance must cover", () => {
  expect(catalog).toHaveLength(12);
});

test.concurrent("check ids are unique and every named plugin is a catalog plugin", () => {
  const ids = checks.map((check) => check.id);
  expect(new Set(ids).size).toBe(ids.length);
  const named = checks.flatMap((check) => (check.plugins === "all" ? [] : check.plugins));
  expect(named.filter((name) => !catalog.includes(name))).toEqual([]);
});

test.concurrent("every catalog plugin has a dedicated actual-host check when all pass", () => {
  const all = coverage(
    checks.map((check) => ({ check, pass: true })),
    catalog,
  );
  expect(Object.entries(all).filter(([, ids]) => ids.length === 0)).toEqual([]);
});

test.concurrent("a plugin whose only checks fail, or are whole-catalog, is uncovered", () => {
  const results = checks.map((check) => ({
    check,
    pass: !(check.plugins !== "all" && check.plugins.includes("jev")),
  }));
  expect(coverage(results, catalog).jev).toEqual([]);
  const wholeCatalog = checks.filter((check) => check.plugins === "all");
  expect(wholeCatalog.length).toBeGreaterThan(0);
  const onlyAll = coverage(
    wholeCatalog.map((check) => ({ check, pass: true })),
    catalog,
  );
  expect(Object.values(onlyAll).every((ids) => ids.length === 0)).toBe(true);
});

test.concurrent("an in-process check never counts toward a plugin's coverage", () => {
  const inProcess: AcceptanceCheck = {
    id: "in-process.jev",
    family: "x",
    plugins: ["jev"],
    evidence: { execution: "in-process", model: "scripted-loopback", service: "live" },
    run: () => Promise.resolve({ pass: true, observed: {} }),
  };
  expect(coverage([{ check: inProcess, pass: true }], ["jev"])).toEqual({ jev: [] });
});

test.concurrent("every live test file on disk is registered, and every registered one exists", () => {
  const onDisk = [
    ...new Bun.Glob("tools/toolu-opencode/src/**/*.live.test.ts").scanSync({ cwd: ROOT }),
  ];
  expect(onDisk.toSorted()).toEqual(LIVE_TEST_FILES.map((entry) => entry.file).toSorted());
  for (const entry of LIVE_TEST_FILES) {
    const source = readFileSync(join(ROOT, entry.file), "utf8");
    for (const external of entry.external ?? []) {
      expect(source).toContain(`"${external.name}"`);
      expect(source).toContain(`process.env.${external.flag} !== "1"`);
    }
  }
});

test.concurrent("selecting an unknown id throws; no ids selects everything", () => {
  expect(() => selectChecks(checks, ["entry.npm-root", "nope.missing"])).toThrow(
    "unknown acceptance check: nope.missing",
  );
  expect(selectChecks(checks, [])).toHaveLength(checks.length);
  expect(selectChecks(checks, ["entry.npm-root"]).map((check) => check.id)).toEqual([
    "entry.npm-root",
  ]);
});

test.concurrent("checks run one at a time, in order", async () => {
  const events: string[] = [];
  const results = await inSequence([3, 1, 2], async (n) => {
    events.push(`start ${n}`);
    await Bun.sleep(n);
    events.push(`end ${n}`);
    return n * 10;
  });
  expect(results).toEqual([30, 10, 20]);
  expect(events).toEqual(["start 3", "end 3", "start 1", "end 1", "start 2", "end 2"]);
});
