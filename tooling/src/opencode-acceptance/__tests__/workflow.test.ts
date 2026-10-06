import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { ROOT } from "../../opencode-host/scenarios-entry.ts";

// The real CI workflow (#362 AC-7): acceptance runs on both declared platforms, can never be
// narrowed or soft-failed, and the required `typescript` status fails unless it passed.

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

const GROUP_OF: Record<string, string> = { ts: "ts", opencode: "opencode" };

test.concurrent("the acceptance job runs the full command on Linux and macOS", () => {
  const opencode = job("opencode");
  expect(opencode.strategy?.matrix.os?.toSorted()).toEqual(["macos-latest", "ubuntu-latest"]);
  const commands = runs("opencode").filter((run) => run.includes("test:opencode"));
  expect(commands).toEqual([
    'bun run test:opencode --report "$RUNNER_TEMP/opencode-acceptance.json"',
  ]);
  expect(commands.join("\n")).not.toContain("--only");
});

// #458: the ts and acceptance jobs skip only through their path group; no step soft-fails.
test.concurrent("no acceptance or ts step may soft-fail or be skipped except by its path group", () => {
  for (const [id, group] of Object.entries(GROUP_OF)) {
    expect(job(id)["continue-on-error"]).toBeUndefined();
    expect(job(id).steps.filter((step) => step["continue-on-error"] !== undefined)).toEqual([]);
    expect(job(id).if).toBe(`needs.changes.outputs.${group} == 'true'`);
    expect(job(id).needs).toBe("changes");
  }
  const acceptance = job("opencode").steps.find((step) => step.run?.includes("test:opencode"));
  expect(acceptance?.if).toBeUndefined();
  expect(runs("ts").join("\n")).toContain("bun run test:ts");
});

test.concurrent("the acceptance job installs ast-grep beyond runner-provided tools", () => {
  const installs = runs("opencode").join("\n");
  expect(installs).toContain("@ast-grep/cli");
});

test.concurrent("the required typescript status needs every gated job, always runs and judges their results", () => {
  const required = job("typescript");
  expect(required.name).toBe("typescript");
  expect([required.needs ?? []].flat().toSorted()).toEqual([
    "changes",
    "docs",
    "hook-bench",
    "opencode",
    "rust",
    "rust-conformance",
    "rust-musl",
    "ts",
  ]);
  expect(required.if).toBe("${{ always() }}");
  expect(runs("typescript")).toContain("bun run tooling/src/ci-aggregate.ts tests.yml");
  const names = Object.entries(workflow.jobs).filter(([, item]) => item.name === "typescript");
  expect(names.map(([id]) => id)).toEqual(["typescript"]);
});
