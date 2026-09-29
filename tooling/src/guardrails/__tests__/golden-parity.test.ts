// Bash→TypeScript parity (#277): every verdict the bash guardrails produced on
// the upstream fixture trees (captured on Linux with GNU grep) must come out of
// the TypeScript runner unchanged — exit code, stdout and stderr lines.
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { z } from "zod";
import { FIXTURES } from "./fixture-tree.ts";
import { runCase } from "./golden-cases.ts";

const RUNNER = ["bun", resolve(import.meta.dir, "../run.ts")];

const Result = z.object({
  exit: z.number(),
  stdout: z.array(z.string()),
  stderr: z.array(z.string()),
});
const Golden = z.object({
  bashSha: z.string().min(7),
  platform: z.literal("linux"),
  cases: z.array(
    z.object({
      id: z.string(),
      fixture: z.enum(["clean", "violating", "workspace", "workspace-violating"]),
      args: z.array(z.string()),
      stdin: z.string(),
      mutate: z.record(z.string(), z.string()).optional(),
      result: Result,
    }),
  ),
});

/**
 * Named, deliberate differences from bash. Each is a bash defect the port does
 * not reproduce; the golden itself stays exactly the way it was captured.
 */
const DEVIATIONS: Readonly<Record<string, { exit: number; why: string }>> = {
  "violating --only filename-case": {
    exit: 1,
    why: "bash reported filename-case from a pipeline subshell, so the violation printed but the gate exited 0",
  },
};

const golden = Golden.parse(JSON.parse(readFileSync(join(FIXTURES, "golden.json"), "utf8")));

test("the golden covers every fixture and mode", () => {
  expect(golden.cases.length).toBeGreaterThanOrEqual(200);
  const fixtures = new Set(golden.cases.map((c) => c.fixture));
  expect([...fixtures].toSorted()).toEqual([
    "clean",
    "violating",
    "workspace",
    "workspace-violating",
  ]);
  expect(golden.cases.map((c) => c.result.exit).toSorted((a, b) => a - b)).toContain(3);
});

for (const c of golden.cases) {
  test(`parity: ${c.id}`, () => {
    const deviation = DEVIATIONS[c.id];
    const expected = deviation === undefined ? c.result : { ...c.result, exit: deviation.exit };
    expect(runCase(RUNNER, c)).toEqual(expected);
  }, 60_000);
}
