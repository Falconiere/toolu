/**
 * The committed hook-bench results (#410 AC-6, AC-8): every one is a valid
 * `toolu.hook-resources/v1` result, and the Bun baseline from the CI runners
 * records measured hook entries on Linux and macOS, the Linux run inside 60 s.
 * New entries are covered by the current payload fixture and the CI hook bench.
 */
import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { HookResult, loadJson } from "../lib/hook-data.ts";

const ROOT = resolve(import.meta.dir, "../../../..");
const RESULTS = join(ROOT, "benchmarks/results");
const files = [...new Bun.Glob("hook-resources-*.json").scanSync({ cwd: RESULTS })].toSorted();
const results = files.map((file) => ({ file, doc: loadJson(join(RESULTS, file), HookResult) }));

test("each committed result is named for its implementation, platform and runner", () => {
  expect(files.length).toBeGreaterThan(0);
  for (const { file, doc } of results) {
    const impl = doc.entries.some((e) => e.implementation === "rust") ? "rust" : "bun";
    const where = doc.provenance.runner === "github-actions" ? "ci" : "local";
    expect(file).toBe(
      `hook-resources-${impl}-${doc.provenance.platform}-${doc.provenance.arch}-${where}-${doc.provenance.date}.json`,
    );
  }
});

test("the dated Bun baselines from CI hold distinct measured entries on Linux and macOS", () => {
  for (const platform of ["linux", "darwin"]) {
    const baseline = results.find(
      ({ doc }) =>
        doc.provenance.platform === platform &&
        doc.provenance.runner === "github-actions" &&
        doc.entries.every((e) => e.implementation === "bun"),
    );
    expect(baseline, platform).toBeDefined();
    // A dated result records what existed when CI measured it.
    const entries = baseline?.doc.entries ?? [];
    expect(entries.length, platform).toBeGreaterThan(0);
    expect(new Set(entries.map((entry) => entry.entry)).size, platform).toBe(entries.length);
    for (const row of baseline?.doc.entries ?? []) {
      expect(row.maxRssBytes.p50, row.entry).toBeGreaterThan(
        baseline?.doc.provenance.floor.maxRssBytes ?? 0,
      );
      expect(row.cpuUs.p50, row.entry).toBeGreaterThan(0);
    }
  }
});

test("the Linux CI bench measured every entry in under 60 s", () => {
  const linux = results.filter(
    ({ doc }) => doc.provenance.platform === "linux" && doc.provenance.runner === "github-actions",
  );
  expect(linux.length).toBeGreaterThan(0);
  for (const { file, doc } of linux) expect(doc.provenance.elapsedMs, file).toBeLessThan(60_000);
});
