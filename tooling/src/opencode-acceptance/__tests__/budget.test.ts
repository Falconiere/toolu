import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { contractPaths } from "../../opencode-host/results.ts";
import { overheads, type BudgetSample } from "../budget.ts";

// Overheads from request arrival times shaped like a recorded pinned-host run (#362 AC-5).

/** A run whose first tool request arrives `startup` ms after spawn, then one every `gap` ms. */
function run(kind: BudgetSample["kind"], startup: number, gaps: readonly number[]): BudgetSample {
  const spawnAt = 1_000_000;
  const times = [spawnAt + startup];
  for (const gap of gaps) times.push((times.at(-1) ?? 0) + gap);
  return { kind, spawnAt, requestTimes: times };
}

test.concurrent("overheads are toolu minus reference, each at its fastest run", () => {
  const samples = [
    run("reference", 4000, [100, 110, 90, 100, 105, 95]),
    run("toolu", 6500, [180, 170, 190, 175, 185, 600]),
    run("reference", 3800, [120, 120, 120, 120, 120, 120]),
    run("toolu", 7000, [160, 165, 170, 175, 180, 185]),
  ];
  // Fastest startups: 6500 and 3800. Fastest medians: toolu 172.5, reference 100.
  expect(overheads(samples)).toEqual({ startupOverheadMs: 2700, perToolOverheadMs: 73 });
});

test.concurrent("a run that made fewer tool requests than scripted is an error, not a fast run", () => {
  const short = [
    run("reference", 4000, [100, 100, 100, 100, 100, 100]),
    run("toolu", 5000, [100, 100]),
  ];
  expect(() => overheads(short)).toThrow("budget toolu run made 3 tool requests, expected 7");
});

test.concurrent("a missing kind is an error", () => {
  expect(() => overheads([run("toolu", 5000, [1, 1, 1, 1, 1, 1])])).toThrow(
    "budget has no reference run",
  );
});

test.concurrent("the committed ceilings are positive numbers", () => {
  const raw: unknown = JSON.parse(
    readFileSync(join(contractPaths().dir, "acceptance-budgets.json"), "utf8"),
  );
  const parsed = z
    .strictObject({
      version: z.literal(1),
      startupOverheadMs: z.number().positive(),
      perToolOverheadMs: z.number().positive(),
    })
    .parse(raw);
  expect(parsed.startupOverheadMs).toBeGreaterThan(parsed.perToolOverheadMs);
});
