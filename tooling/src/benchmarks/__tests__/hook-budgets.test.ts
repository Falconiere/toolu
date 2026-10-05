/**
 * The hook budgets follow from the committed prototype measurement by the rule
 * in docs/resource-budgets.md (#410 AC-7), and the doc states the same numbers.
 */
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { z } from "zod";
import { Budgets, loadJson } from "../lib/hook-data.ts";
import { MIB } from "../lib/hook-report.ts";

const ROOT = resolve(import.meta.dir, "../../../..");

const Spread = z.strictObject({ p50: z.number().positive(), p90: z.number().positive() });
const Hook = z.strictObject({ maxRssBytes: Spread, cpuUs: Spread, wallUs: Spread });
const Prototype = z.looseObject({
  schema: z.literal("toolu.hook-prototype/v1"),
  prototype: z.looseObject({
    shape: z.array(z.string()).min(1),
    dependencies: z.record(z.string(), z.string()),
  }),
  binaryBytes: z.number().positive(),
  hooks: z.strictObject({
    fastPath: z.record(z.string(), Hook),
    fullClapTree: z.record(z.string(), Hook),
  }),
});

/** The epic's starting budgets, which the prototype measurement refines. */
const START: Record<string, { maxRssMiB: number; cpuMs: number }> = {
  "toolu/pre-tools": { maxRssMiB: 8, cpuMs: 5 },
  "toolu/post-tools": { maxRssMiB: 10, cpuMs: 10 },
};

/** RSS tightens to p50 × 1.5; CPU keeps the start unless the prototype alone exceeds it. */
function rule(start: number, measured: number, tighten: boolean): number {
  if (measured > start) return Math.ceil(measured * 1.25);
  return tighten ? Math.min(start, Math.ceil(measured * 1.5)) : start;
}

const proto = loadJson(join(ROOT, "benchmarks/results/hook-prototype-2026-10-05.json"), Prototype);
const budgets = loadJson(join(ROOT, "benchmarks/hook-budgets.json"), Budgets);

test("every hook budget is the rule applied to the prototype's p50", () => {
  expect(Object.keys(budgets.entries).toSorted()).toEqual(Object.keys(START).toSorted());
  for (const [entry, start] of Object.entries(START)) {
    const measured = proto.hooks.fastPath[entry];
    expect(measured, entry).toBeDefined();
    if (measured === undefined) continue;
    expect(budgets.entries[entry], entry).toEqual({
      maxRssMiB: rule(start.maxRssMiB, measured.maxRssBytes.p50 / MIB, true),
      cpuMs: rule(start.cpuMs, measured.cpuUs.p50 / 1000, false),
    });
  }
});

test("the fast path is what the budgets were measured with, and it is cheaper", () => {
  for (const entry of Object.keys(START)) {
    const fast = proto.hooks.fastPath[entry];
    const full = proto.hooks.fullClapTree[entry];
    expect(fast && full && fast.cpuUs.p50 < full.cpuUs.p50, entry).toBe(true);
  }
});

test("docs/resource-budgets.md states every hook budget", () => {
  const doc = readFileSync(join(ROOT, "docs/resource-budgets.md"), "utf8");
  for (const [entry, budget] of Object.entries(budgets.entries)) {
    const name = `toolu hook ${entry.replace("toolu/", "")}`;
    expect(doc, entry).toContain(
      `| \`${name}\` | ≤ ${String(budget.maxRssMiB)} MiB max RSS, ≤ ${String(budget.cpuMs)} ms CPU (p50)`,
    );
  }
  expect(doc).toContain(`prototype: ${(proto.binaryBytes / MIB).toFixed(2)} MiB`);
});
