/**
 * The hook bench's data and logic (#410), against the repository's real
 * hooks.json files and committed data. End-to-end runs, which need the native
 * measurer, are in hook-resources.native.test.ts.
 */
import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { bundlePath } from "@toolu/conformance/harness/entry-command";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { parseArgs } from "../lib/hook-args.ts";
import {
  BenchError,
  Budgets,
  type EntryResult,
  MeasureReport,
  Payloads,
  loadJson,
} from "../lib/hook-data.ts";
import { discoverEntries } from "../lib/hook-entries.ts";
import { payloadStdin } from "../lib/hook-fixture.ts";
import { buildMeasurer } from "../lib/hook-measurer.ts";
import { MIB, summarize, violations } from "../lib/hook-report.ts";

const ROOT = resolve(import.meta.dir, "../../../..");
const ENTRIES = discoverEntries(join(ROOT, "plugins"));
const PAYLOADS = loadJson(join(ROOT, "benchmarks/cases/hooks/payloads.json"), Payloads);

test("discovery finds every hooks.json bundle entry once, register included per plugin", () => {
  const ids = ENTRIES.map((e) => e.id);
  expect(ids).toHaveLength(27);
  expect(new Set(ids).size).toBe(ids.length);
  expect(ids).toContain("toolu/pre-tools");
  expect(ids).toContain("toolu/post-tools");
  expect(ids.filter((id) => id.endsWith("/register"))).toEqual([
    "ast-grep/register",
    "python-quality/register",
    "rust-quality/register",
    "ts-quality/register",
  ]);
  for (const entry of ENTRIES) expect(entry.command).toContain(bundlePath("", entry.entry));
});

test("the committed payloads cover exactly the discovered entries, each for its event", () => {
  expect(Object.keys(PAYLOADS.entries).toSorted()).toEqual(ENTRIES.map((e) => e.id));
  for (const entry of ENTRIES) {
    expect(PAYLOADS.entries[entry.id]?.stdin.hook_event_name, entry.id).toBe(entry.event);
  }
});

test("payload strings name the sandbox project", () => {
  const stdin = payloadStdin(
    { cwd: "${PROJECT}", tool_input: { file_path: "${PROJECT}/README.md" }, n: 1 },
    "/tmp/p",
  );
  expect(JSON.parse(stdin)).toEqual({
    cwd: "/tmp/p",
    tool_input: { file_path: "/tmp/p/README.md" },
    n: 1,
  });
});

test("flags parse, and bad flags are usage errors", () => {
  const args = parseArgs(
    ["--runs", "3", "--warmup", "0", "--only", "toolu/pre-tools", "--assert"],
    ROOT,
  );
  expect(args).toMatchObject({ runs: 3, warmup: 0, only: ["toolu/pre-tools"], assert: true });
  expect(args.manifest).toBe(join(ROOT, "fixtures/rust-ported.json"));
  for (const bad of [
    ["--runs", "0"],
    ["--runs", "1.5"],
    ["--warmup", "-1"],
    ["--nope", "x"],
    ["--out"],
  ]) {
    expect(() => parseArgs(bad, ROOT), bad.join(" ")).toThrow(BenchError);
  }
});

const row = (entry: string, rssMiB: number, cpuMs: number): EntryResult => ({
  entry,
  event: "PreToolUse",
  implementation: "rust",
  maxRssBytes: { p50: rssMiB * MIB, p90: rssMiB * MIB },
  cpuUs: { p50: cpuMs * 1000, p90: cpuMs * 1000 },
  wallUs: { p50: 1, p90: 1 },
});

test("a ported entry over budget yields the measured and budgeted values; others are ignored", () => {
  const budgets = Budgets.parse({
    version: 1,
    percentile: 50,
    entries: { "toolu/pre-tools": { maxRssMiB: 8, cpuMs: 5 } },
  });
  const rows = [
    row("toolu/pre-tools", 9.25, 12.4),
    row("toolu/agent-tier", 99, 99),
    row("toolu/post-tools", 1, 1),
  ];
  const asserted = new Set(["toolu/pre-tools", "toolu/agent-tier"]);
  expect(violations(rows, asserted, budgets, "budgets.json")).toEqual([
    "toolu/pre-tools [rust]: cpu p50 12.4 ms > budget 5 ms",
    "toolu/pre-tools [rust]: rss p50 9.3 MiB > budget 8 MiB",
    "toolu/agent-tier [rust]: no budget in budgets.json",
  ]);
  expect(violations([row("toolu/pre-tools", 8, 5)], asserted, budgets, "b")).toEqual([]);
});

function sample(n: number): MeasureReport {
  return {
    version: 1,
    command: ["x"],
    exitCode: 0,
    signal: null,
    wallUs: n * 10,
    userUs: n,
    sysUs: n,
    maxRssBytes: n * 100,
  };
}

test("summaries are nearest-rank p50 and p90 of RSS, user + sys CPU and wall", () => {
  const summary = summarize([1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map(sample));
  expect(summary).toEqual({
    maxRssBytes: { p50: 500, p90: 900 },
    cpuUs: { p50: 10, p90: 18 },
    wallUs: { p50: 50, p90: 90 },
  });
});

test("the committed budgets file is strict", () => {
  const budgets = loadJson(join(ROOT, "benchmarks/hook-budgets.json"), Budgets);
  expect(Object.keys(budgets.entries)).toContain("toolu/pre-tools");
  expect(Budgets.safeParse({ ...budgets, extra: 1 }).success).toBe(false);
  expect(
    Budgets.safeParse({
      version: 1,
      percentile: 50,
      entries: { "a/b": { maxRssMiB: 0, cpuMs: 1 } },
    }).success,
  ).toBe(false);
});

test("a ported entry with no hooks.json launcher fails before anything is measured", async () => {
  using sb = createSandbox();
  const manifest = sb.write("rust-ported.json", {
    entries: ["toolu/pre-tools", "toolu/plan-ledger", "toolu/nope"],
  });
  const result = await run(
    [
      process.execPath,
      join(ROOT, "tooling/src/benchmarks/hook-resources.ts"),
      "--assert",
      "--manifest",
      manifest,
    ],
    // No cargo: the run must stop before it builds the measurer.
    { cwd: ROOT, env: { CARGO: "/no/such/cargo" } },
  );
  expect(result.exitCode).toBe(2);
  // A skill CLI entry is ported without a launcher; only the unknown one is named.
  expect(result.stderr).toContain("ported entries with no hooks.json launcher: toolu/nope\n");
});

test("a malformed hooks.json or measurer report is a setup error naming the file", () => {
  using sb = createSandbox();
  sb.write("plugins/broken/hooks/hooks.json", "{ not json");
  expect(() => discoverEntries(sb.path("plugins"))).toThrow(BenchError);
  expect(() => discoverEntries(sb.path("plugins"))).toThrow(
    sb.path("plugins/broken/hooks/hooks.json"),
  );
  const report = sb.write("report.json", { version: 1, command: ["true"], exitCode: 0 });
  let caught: unknown;
  try {
    loadJson(report, MeasureReport);
  } catch (error) {
    caught = error;
  }
  expect(caught).toBeInstanceOf(BenchError);
  expect(caught instanceof BenchError && caught.exitCode).toBe(2);
  expect(String(caught)).toContain(`${report} is malformed`);
});

test("the measurer path is absolute even for a relative CARGO_TARGET_DIR", () => {
  // `true` stands in for an up-to-date `cargo build`; only the returned path is under test.
  const path = buildMeasurer(ROOT, { CARGO: "true", CARGO_TARGET_DIR: "target-alt" });
  expect(path).toBe(join(ROOT, "target-alt", "release", "xtask"));
  expect(() => buildMeasurer(ROOT, { CARGO: "false" })).toThrow(
    "cargo build --release -p xtask failed",
  );
});
