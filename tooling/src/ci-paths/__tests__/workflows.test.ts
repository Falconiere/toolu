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
    "npx-invocation.test.ts",
  ]) {
    expect(docs).toContain(part);
  }
  expect(docs).not.toContain("test:unit");
  expect(docs).not.toContain("test:opencode");
});

test.concurrent("review runs for any non-release change and fails open on a broken changes job (AC-1, AC-2)", () => {
  expect(workflow("toolu-review.yml").jobs.review?.if).toBe(
    "${{ !cancelled() && (needs.changes.result != 'success' || needs.changes.outputs.changed != 'false' || github.event.pull_request.head.repo.full_name != github.repository) && (github.head_ref != 'release-please--branches--main--components--toolu' || github.event.pull_request.head.repo.full_name != github.repository) }}",
  );
});

test.concurrent("the aggregate receives every needed job's result", () => {
  for (const id of ["gate", "typescript"]) {
    const judge = steps("tests.yml", id).find((step) => step.run?.includes("ci-aggregate"));
    expect(judge?.env?.NEEDS).toBe("${{ toJSON(needs) }}");
  }
});

test.concurrent("the full gate runs the CI path check", () => {
  expect(scripts["test:ts"]).toContain("bun run check:ci-paths");
});

test.concurrent("both required aggregates and Rust OS checks retain their status names", () => {
  const tests = workflow("tests.yml");
  const needed = [
    "changes",
    "ts",
    "opencode",
    "docs",
    "rust",
    "rust-musl",
    "fuzz",
    "rust-conformance",
    "hook-bench",
  ];
  expect(tests.jobs.gate?.needs).toEqual(needed);
  expect(tests.jobs.typescript?.needs).toEqual(needed);
  const RustMatrix = z.looseObject({
    strategy: z.looseObject({
      matrix: z.looseObject({
        include: z.array(z.looseObject({ os: z.string(), check: z.string() })),
      }),
    }),
  });
  expect(RustMatrix.parse(tests.jobs.rust).strategy.matrix.include).toEqual([
    { os: "ubuntu-latest", check: "rust" },
    { os: "macos-14", check: "rust-macos" },
  ]);
});

test.concurrent("the Rust jobs run cargo xtask gate and both musl targets (#407 AC-5, AC-7; #455)", () => {
  const tests = workflow("tests.yml");
  for (const id of ["rust", "rust-musl", "fuzz"]) {
    expect(tests.jobs[id]?.if).toBe("needs.changes.outputs.rust == 'true'");
    expect(config.workflows["tests.yml"]?.jobs[id]).toBe("rust");
  }
  const Matrix = z.looseObject({
    strategy: z.looseObject({
      matrix: z.looseObject({
        os: z.array(z.string()).optional(),
        include: z.array(z.looseObject({ target: z.string(), os: z.string() })).optional(),
      }),
    }),
  });
  expect(Matrix.parse(tests.jobs["rust-musl"]).strategy.matrix.include).toEqual([
    { target: "x86_64-unknown-linux-musl", os: "ubuntu-latest" },
    { target: "aarch64-unknown-linux-musl", os: "ubuntu-24.04-arm" },
  ]);
  const rust = steps("tests.yml", "rust");
  const cargo = rust.map((step) => step.run ?? "");
  const mac = rust.find((step) => step.run === "cargo xtask gate --only clippy --only tests");
  expect(mac?.if).toBe("matrix.os == 'macos-14'");
  expect(cargo).toContain("rustup toolchain install");
  expect(cargo).toContain("bun install --frozen-lockfile");
  expect(cargo).toContain("npm install -g @ast-grep/cli");
  // #455: the job runs the whole quality bar through `cargo xtask gate`.
  expect(cargo.some((run) => run.includes("cargo xtask gate "))).toBe(true);
  expect(JSON.stringify(rust)).toContain(
    "cargo-deny@0.20.2,cargo-machete@0.9.2,cargo-llvm-cov@0.9.1",
  );
  const musl = steps("tests.yml", "rust-musl").map((step) => step.run ?? "");
  expect(musl).toContain(
    'cargo build --workspace --release --locked --target "${{ matrix.target }}"',
  );
  expect(musl.some((run) => run.includes("grep -Eq '(static-pie|statically) linked'"))).toBe(true);
  // The product binary is the one that must be static (#411).
  expect(musl.some((run) => run.includes('release/toolu")'))).toBe(true);
  // #412: the launcher finds toolu at Homebrew's bin and at /usr/local/bin.
  const e2e = cargo.find((run) => run.includes("cargo xtask launcher-e2e")) ?? "";
  for (const part of [
    "cargo build --release -p toolu-cli --locked",
    "brew_bin=/opt/homebrew/bin",
    "brew_bin=/home/linuxbrew/.linuxbrew/bin",
    'for dir in "$brew_bin" /usr/local/bin',
    'sudo install -m 0755 target/release/toolu "$dir/toolu"',
    'cargo xtask launcher-e2e --bin "$dir/toolu"',
    'sudo rm -f "$dir/toolu"',
  ]) {
    expect(e2e).toContain(part);
  }
});

test.concurrent("the Rust conformance leg reads the port list and no-ops when it is empty (#409 AC-5)", () => {
  const job = workflow("tests.yml").jobs["rust-conformance"];
  expect(job?.if).toBe("needs.changes.outputs.ports == 'true'");
  expect(config.workflows["tests.yml"]?.jobs["rust-conformance"]).toBe("ports");
  expect(config.groups.ports).toContain("fixtures/**");
  const runs = steps("tests.yml", "rust-conformance").map((step) => step.run);
  expect(runs).toContain("bun run test:rust-conformance");
  expect(scripts["test:rust-conformance"]).toBe("bun run tooling/src/rust-conformance.ts");
  const Gated = z.looseObject({ if: z.string().optional(), run: z.string().optional() });
  const toolchain = Steps.parse(job).steps.map((step) => Gated.parse(step));
  expect(toolchain.find((step) => step.run === "rustup toolchain install")?.if).toBe(
    "steps.ports.outputs.count != '0'",
  );
});

test.concurrent("hook-bench measures on Linux and macOS and asserts budgets on Linux only (#410 AC-8)", () => {
  const job = workflow("tests.yml").jobs["hook-bench"];
  expect(job?.if).toBe("needs.changes.outputs.ports == 'true'");
  expect(config.workflows["tests.yml"]?.jobs["hook-bench"]).toBe("ports");
  for (const path of ["benchmarks/hook-budgets.json", "tooling/src/benchmarks/hook-resources.ts"]) {
    expect(config.groups.ports).toContain(path);
  }
  const Matrix = z.looseObject({
    strategy: z.looseObject({ matrix: z.looseObject({ os: z.array(z.string()) }) }),
  });
  expect(Matrix.parse(job).strategy.matrix.os).toEqual(["ubuntu-latest", "macos-latest"]);
  const all = steps("tests.yml", "hook-bench");
  const runs = all.map((step) => step.run ?? "");
  expect(runs).toContain(
    "bun test --timeout 180000 tooling/src/benchmarks/__tests__/hook-resources.native.test.ts",
  );
  const bench = runs.find((run) => run.includes("bun run bench:hooks"));
  expect(bench).toContain('if [ "$RUNNER_OS" = "Linux" ]; then assert=--assert; fi');
  expect(bench).toContain('--out "$RESULT" $assert');
  expect(scripts["bench:hooks"]).toBe("bun run tooling/src/benchmarks/hook-resources.ts");
  expect(JSON.stringify(all)).toContain("actions/upload-artifact@v4");
});

test.concurrent("toolu-shell is fuzzed on every Rust change and on a schedule (#416 AC-5)", () => {
  const perChange = steps("tests.yml", "fuzz").map((step) => step.run ?? "");
  for (const target of ["analyze", "nested"]) {
    expect(perChange).toContain(
      `cargo fuzz run ${target} --target x86_64-unknown-linux-gnu -- -max_total_time=60 -timeout=10 -rss_limit_mb=4096`,
    );
  }
  const scheduled = workflow("fuzz.yml");
  expect(JSON.stringify(scheduled)).toContain("max_total_time=1800");
  const latency = steps("tests.yml", "rust").map((step) => step.run ?? "");
  expect(latency.some((run) => run.includes("--test latency"))).toBe(true);
  const musl = steps("tests.yml", "rust-musl").map((step) => step.run ?? "");
  expect(musl.some((run) => run.includes('=musl-gcc" >> "$GITHUB_ENV"'))).toBe(true);
});
