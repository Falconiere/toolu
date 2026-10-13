import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { ROOT } from "../../opencode-host/scenarios-entry.ts";

// OpenCode acceptance is not a Tests job. The required `typescript` status still
// aggregates every remaining gated job.

const Step = z.looseObject({
  name: z.string().optional(),
  run: z.string().optional(),
  uses: z.string().optional(),
  if: z.string().optional(),
  "continue-on-error": z.unknown().optional(),
});
const Job = z.looseObject({
  name: z.string(),
  needs: z.union([z.string(), z.array(z.string())]).optional(),
  if: z.string().optional(),
  "continue-on-error": z.unknown().optional(),
  // `os` is optional: rust-musl (#407) builds its matrix from `include` entries.
  strategy: z
    .looseObject({ matrix: z.looseObject({ os: z.array(z.string()).optional() }) })
    .optional(),
  steps: z.array(Step),
});
const Workflow = z.looseObject({ jobs: z.record(z.string(), Job) });

const workflow = Workflow.parse(
  Bun.YAML.parse(readFileSync(join(ROOT, ".github/workflows/tests.yml"), "utf8")),
);

function job(id: string): z.infer<typeof Job> {
  const found = workflow.jobs[id];
  if (found === undefined) throw new Error(`tests.yml has no ${id} job`);
  return found;
}

const runs = (id: string): string[] => job(id).steps.flatMap((step) => step.run ?? []);

test.concurrent("Tests does not run OpenCode acceptance", () => {
  expect(workflow.jobs.opencode).toBeUndefined();
  expect(JSON.stringify(workflow)).not.toContain("test:opencode");
});

test.concurrent("the required typescript status needs every gated job, always runs and judges their results", () => {
  const required = job("typescript");
  expect(required.name).toBe("typescript");
  expect([required.needs ?? []].flat().toSorted()).toEqual([
    "changes",
    "docs",
    "fuzz",
    "hook-bench",
    "rust",
    "rust-conformance",
    "rust-musl",
  ]);
  expect(required.if).toBe("${{ always() }}");
  expect(runs("typescript")).toContain("cargo xtask ci-aggregate tests.yml");
  const names = Object.entries(workflow.jobs).filter(([, item]) => item.name === "typescript");
  expect(names.map(([id]) => id)).toEqual(["typescript"]);
});
