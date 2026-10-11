#!/usr/bin/env bun
/** Structural contract for the Rust CI, review, and native release workflows. */
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import {
  type ObjectMap,
  object,
  at,
  string,
  array,
  steps,
  runs,
  includes,
  need,
  checkRelease,
} from "./workflow-checks.ts";
import { checkInstallChannels } from "./install-channel-checks.ts";

function workflow(root: string, name: string): ObjectMap {
  return object(Bun.YAML.parse(readFileSync(join(root, ".github/workflows", name), "utf8")));
}

function checkTriggers(docs: ReadonlyMap<string, ObjectMap>, errors: string[]): void {
  const events = (name: string): ObjectMap => object(at(docs.get(name), "on"));
  const has = (name: string, event: string): boolean => Object.hasOwn(events(name), event);
  const main = (name: string, event: string): boolean =>
    array(at(events(name), event, "branches")).includes("main");
  const unfiltered = (name: string, event: string): boolean =>
    at(events(name), event, "paths") === undefined &&
    at(events(name), event, "paths-ignore") === undefined;
  need(
    errors,
    main("tests.yml", "push") &&
      main("tests.yml", "pull_request") &&
      has("tests.yml", "workflow_dispatch") &&
      unfiltered("tests.yml", "push") &&
      unfiltered("tests.yml", "pull_request"),
    "tests.yml must run on main push/PR and manual dispatch without path filters",
  );
  need(
    errors,
    array(at(events("toolu-review.yml"), "pull_request", "types")).includes("synchronize") &&
      unfiltered("toolu-review.yml", "pull_request"),
    "toolu-review.yml must review synchronized PRs without path filters",
  );
  need(
    errors,
    main("release-please.yml", "push") &&
      has("release-please.yml", "workflow_dispatch") &&
      has("release-native.yml", "workflow_call") &&
      has("release-native.yml", "workflow_dispatch") &&
      has("release-finalize.yml", "workflow_call"),
    "release workflows must retain push, reusable, and dry-run triggers",
  );
}

function checkActionsAndPermissions(name: string, doc: ObjectMap, errors: string[]): void {
  const allowedWrites = new Set([
    "release-please.yml:release-please",
    "release-please.yml:native",
    "release-native.yml:upload",
    "release-native.yml:publish",
  ]);
  const jobs = object(doc.jobs);
  const defaultContents = at(doc, "permissions", "contents");
  need(
    errors,
    defaultContents === "read",
    `${name} must declare least-privilege default contents permission`,
  );
  const holders: [string, ObjectMap][] = [
    ["*", doc],
    ...Object.entries(jobs).map(([id, job]): [string, ObjectMap] => [id, object(job)]),
  ];
  for (const [id, holder] of holders) {
    const permissions = object(holder.permissions);
    need(
      errors,
      permissions.contents !== "write" || allowedWrites.has(`${name}:${id}`),
      `${name}:${id} has unnecessary contents: write`,
    );
    const actions = id === "*" ? [] : [holder, ...steps(holder)];
    for (const action of actions) {
      const use = string(action.uses);
      if (use === "" || use.startsWith("./")) continue;
      need(
        errors,
        /@(?:v\d+(?:\.\d+)*|[a-f0-9]{40})$/.test(use),
        `${name}:${id} action ${use} is not version-pinned`,
      );
    }
  }
}

function checkCiAggregates(tests: ObjectMap, errors: string[]): void {
  const jobs = object(tests.jobs);
  for (const id of ["gate", "typescript"]) {
    const job = object(jobs[id]);
    const deps = array(job.needs).map(string).toSorted();
    const expected = [
      "changes",
      "opencode",
      "docs",
      "rust",
      "rust-musl",
      "fuzz",
      "rust-conformance",
      "hook-bench",
    ].toSorted();
    need(
      errors,
      JSON.stringify(deps) === JSON.stringify(expected) && includes(job.if, "always()"),
      `tests.yml:${id} must aggregate every gated job with always()`,
    );
    need(errors, job.name === id, `tests.yml:${id} must report the ${id} check`);
  }
  need(errors, !("ts" in jobs), "tests.yml must not keep the ts job");
  need(
    errors,
    runs(object(jobs.changes)).includes("cargo xtask ci-changes"),
    "tests.yml:changes must run cargo xtask ci-changes",
  );
  for (const id of ["gate", "typescript"]) {
    need(
      errors,
      runs(object(jobs[id])).includes("cargo xtask ci-aggregate tests.yml"),
      `tests.yml:${id} must run cargo xtask ci-aggregate`,
    );
  }
  need(
    errors,
    !JSON.stringify(tests).includes("bun run test:ts"),
    "tests.yml must not run bun run test:ts",
  );
}

function checkRustCi(tests: ObjectMap, errors: string[]): void {
  const jobs = object(tests.jobs);
  const rust = object(jobs.rust);
  need(
    errors,
    includes(rust.name, "matrix.check"),
    "tests.yml:rust must report each OS check name",
  );
  const legs = array(at(rust, "strategy", "matrix", "include"));
  for (const [os, check] of [
    ["ubuntu-latest", "rust"],
    ["macos-14", "rust-macos"],
  ] as const) {
    need(
      errors,
      legs.some((leg) => at(leg, "os") === os && at(leg, "check") === check),
      `tests.yml:rust lacks ${check} on ${os}`,
    );
  }
  const command = runs(rust);
  const linuxSteps = [
    "Reject Rust suppressions and structural violations",
    "Check Rust crate layers",
    "cargo xtask gate",
  ];
  need(
    errors,
    linuxSteps.every((name) =>
      steps(rust).some((step) => step.name === name && step.if === "matrix.os == 'ubuntu-latest'"),
    ),
    "tests.yml:rust Linux leg must own the full quality gate",
  );
  need(
    errors,
    steps(rust).some(
      (step) =>
        step.name === "macOS clippy and Rust tests" &&
        step.if === "matrix.os == 'macos-14'" &&
        includes(step.run, "cargo xtask gate --only clippy --only tests"),
    ),
    "tests.yml:rust macOS leg must run clippy and Rust tests",
  );
  for (const required of [
    "cargo xtask gate --only guardrails",
    "cargo xtask gate --only layers",
    "cargo xtask gate",
    "@ast-grep/cli",
    "python3 -B -m unittest discover -s .github/scripts",
  ] as const) {
    need(errors, command.includes(required), `tests.yml:rust lacks ${required}`);
  }
  checkMuslCi(jobs, errors);
}

function checkMuslCi(jobs: ObjectMap, errors: string[]): void {
  need(
    errors,
    includes(at(jobs, "rust-musl", "if"), "needs.changes.outputs.rust"),
    "tests.yml:rust-musl must follow rust paths",
  );
  need(
    errors,
    runs(jobs["rust-musl"]).includes("musl-tools"),
    "tests.yml:rust-musl needs the C toolchain",
  );
  for (const target of ["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"]) {
    need(
      errors,
      array(at(jobs, "rust-musl", "strategy", "matrix", "include")).some(
        (leg) => at(leg, "target") === target,
      ),
      `tests.yml:rust-musl lacks ${target}`,
    );
  }
}

function checkReview(root: string, review: ObjectMap, errors: string[]): void {
  const jobs = object(review.jobs);
  const condition = string(at(jobs, "review", "if"));
  const branch = "release-please--branches--main--components--toolu";
  const sameRepo = "github.event.pull_request.head.repo.full_name != github.repository";
  need(
    errors,
    condition.includes(`github.head_ref != '${branch}' || ${sameRepo}`),
    "toolu-review.yml:review must exempt only the same-repo release PR",
  );
  const action = steps(jobs.review).find((step) => includes(step.uses, "code-review@v8"));
  need(
    errors,
    string(at(action, "with", "CODEBASE_OVERVIEW")).trim() !== "" &&
      at(action, "with", "RULES_MAX_BYTES") === "65536",
    "toolu-review.yml must supply crate context and the full rules budget",
  );
  need(
    errors,
    existsSync(join(root, ".github/code-review-prompt.md")),
    "toolu-review.yml Rust prompt file is missing",
  );
  for (const [key, value] of [
    ["REVIEW_PROMPT_FILE", ".github/code-review-prompt.md"],
    ["EXCLUDE_GLOBS", "fixtures/**"],
    ["EXCLUDE_GLOBS", "docs/cli/**"],
  ] as const) {
    need(
      errors,
      includes(at(action, "with", key), value),
      `toolu-review.yml:review lacks ${key} ${value}`,
    );
  }
}

/** Return one finding per violated invariant, so fixture mutations name the break. */
export function checkWorkflows(root: string): string[] {
  const names = [
    "tests.yml",
    "toolu-review.yml",
    "release-please.yml",
    "release-native.yml",
    "release-homebrew.yml",
    "release-finalize.yml",
    "advisory-audit.yml",
    "npm-publish.yml",
  ];
  const errors: string[] = [];
  const docs = new Map<string, ObjectMap>();
  for (const name of names) {
    try {
      const doc = workflow(root, name);
      docs.set(name, doc);
      checkActionsAndPermissions(name, doc, errors);
    } catch (error) {
      errors.push(`${name}: cannot read or parse: ${String(error)}`);
    }
  }
  const get = (name: string): ObjectMap => docs.get(name) ?? {};
  checkTriggers(docs, errors);
  checkCiAggregates(get("tests.yml"), errors);
  checkRustCi(get("tests.yml"), errors);
  checkReview(root, get("toolu-review.yml"), errors);
  try {
    checkRelease(root, docs, errors);
    checkInstallChannels(root, docs, errors);
  } catch (error) {
    errors.push(`release-please-config.json: cannot read or parse: ${String(error)}`);
  }
  return errors;
}

if (import.meta.main) {
  const root = resolve(process.argv[2] ?? ".");
  const errors = checkWorkflows(root);
  if (errors.length > 0) {
    process.stderr.write(`${errors.map((error) => `check-workflows: ${error}`).join("\n")}\n`);
    process.exitCode = 1;
  } else {
    process.stdout.write("check-workflows: workflows match the Rust release contract\n");
  }
}
