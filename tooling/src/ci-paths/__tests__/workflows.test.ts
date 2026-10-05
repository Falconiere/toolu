import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { z } from "zod";
import { loadRepoCiPaths } from "../config.ts";
import { readWorkflows, type WorkflowFile } from "../workflow.ts";

// The real workflow wiring (#458): what the `changes` job runs and outputs, how
// docs and review are gated, and what the aggregate is given.

const ROOT = resolve(import.meta.dir, "../../../..");
const config = loadRepoCiPaths(ROOT);
const { workflows, errors } = readWorkflows(join(ROOT, ".github/workflows"));

const Step = z.looseObject({
  run: z.string().optional(),
  uses: z.string().optional(),
  with: z.record(z.string(), z.unknown()).optional(),
  env: z.record(z.string(), z.string()).optional(),
});
const Steps = z.looseObject({ steps: z.array(Step) });

function workflow(file: string): WorkflowFile {
  const found = workflows.find((item) => item.file === file);
  if (found === undefined) throw new Error(`no ${file}`);
  return found;
}

function steps(file: string, id: string): z.infer<typeof Step>[] {
  return Steps.parse(workflow(file).jobs[id]).steps;
}

const scripts = z
  .looseObject({ scripts: z.record(z.string(), z.string()) })
  .parse(JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8"))).scripts;

test.concurrent("every workflow parses", () => {
  expect(errors).toEqual([]);
});

for (const file of Object.keys(config.workflows)) {
  test.concurrent(`${file} classifies the full history with ci-changes`, () => {
    const job = workflow(file).jobs.changes;
    const groups = [...new Set(Object.values(config.workflows[file]?.jobs ?? {}))];
    for (const group of groups) {
      expect(job?.outputs?.[group]).toBe(`\${{ steps.groups.outputs.${group} }}`);
    }
    const checkout = steps(file, "changes").find((step) => step.uses === "actions/checkout@v4");
    expect(checkout?.with?.["fetch-depth"]).toBe(0);
    expect(steps(file, "changes").map((step) => step.run)).toContain(
      "bun run tooling/src/ci-changes.ts",
    );
  });
}

test.concurrent("the docs job runs only the documentation checks (AC-1)", () => {
  expect(workflow("tests.yml").jobs.docs?.if).toBe("needs.changes.outputs.docs == 'true'");
  expect(steps("tests.yml", "docs").map((step) => step.run)).toContain("bun run test:docs");
  const docs = scripts["test:docs"] ?? "";
  for (const check of ["check-portable-core-doc", "check:opencode-docs"]) {
    expect(scripts["test:portable-core"]).toContain(check);
  }
  for (const part of [
    "guardrails",
    "test:portable-core",
    "test:gate-coverage",
    "test:final-removal",
    "check:ci-paths",
    "check:opencode-surface",
    "workspace-skeleton.test.ts",
    "plan-ledger-contract.test.ts",
    "deprecation-banners.test.ts",
    "npx-invocation.test.ts",
  ]) {
    expect(docs).toContain(part);
  }
  expect(docs).not.toContain("test:unit");
  expect(docs).not.toContain("test:opencode");
});

test.concurrent("review runs for any non-release change and fails open on a broken changes job (AC-1, AC-2)", () => {
  expect(workflow("toolu-review.yml").jobs.review?.if).toBe(
    "${{ !cancelled() && (needs.changes.result != 'success' || needs.changes.outputs.changed != 'false') }}",
  );
});

test.concurrent("the aggregate receives every needed job's result", () => {
  const judge = steps("tests.yml", "typescript").find((step) => step.run?.includes("ci-aggregate"));
  expect(judge?.env?.NEEDS).toBe("${{ toJSON(needs) }}");
});

test.concurrent("the full gate runs the CI path check", () => {
  expect(scripts["test:ts"]).toContain("bun run check:ci-paths");
});

test.concurrent("the Rust jobs run the full cargo gate and both musl targets (#407 AC-5, AC-7)", () => {
  const tests = workflow("tests.yml");
  for (const id of ["rust", "rust-musl"]) {
    expect(tests.jobs[id]?.if).toBe("needs.changes.outputs.rust == 'true'");
    expect(config.workflows["tests.yml"]?.jobs[id]).toBe("rust");
  }
  expect(tests.jobs.typescript?.needs).toEqual(
    expect.arrayContaining(["changes", "gate", "opencode", "docs", "rust", "rust-musl"]),
  );
  const Matrix = z.looseObject({
    strategy: z.looseObject({
      matrix: z.looseObject({
        os: z.array(z.string()).optional(),
        include: z.array(z.looseObject({ target: z.string(), os: z.string() })).optional(),
      }),
    }),
  });
  expect(Matrix.parse(tests.jobs.rust).strategy.matrix.os).toEqual([
    "ubuntu-latest",
    "macos-latest",
  ]);
  expect(Matrix.parse(tests.jobs["rust-musl"]).strategy.matrix.include).toEqual([
    { target: "x86_64-unknown-linux-musl", os: "ubuntu-latest" },
    { target: "aarch64-unknown-linux-musl", os: "ubuntu-24.04-arm" },
  ]);
  const cargo = steps("tests.yml", "rust").map((step) => step.run);
  expect(cargo).toEqual(
    expect.arrayContaining([
      "rustup toolchain install",
      "cargo fmt --all --check",
      "cargo clippy --workspace --all-targets --locked -- -D warnings",
      "cargo build --workspace --locked",
      "cargo test --workspace --locked",
      "cargo xtask check-layers",
    ]),
  );
  const musl = steps("tests.yml", "rust-musl").map((step) => step.run ?? "");
  expect(musl).toContain(
    'cargo build --workspace --release --locked --target "${{ matrix.target }}"',
  );
  expect(musl.some((run) => run.includes("grep -Eq 'static(-pie)? linked'"))).toBe(true);
});
