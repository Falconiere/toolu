/** Plan-ledger reference prose contract: each private phase carries the marker its
 * phase is responsible for, so the plan↔execution ledger contract stays wired into
 * the prose, not just the scripts. */

import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..", "..", "..", "..");
const REFS = join(ROOT, "plugins", "delivery-flow", "skills", "delivery-flow", "references");
const ref = (name: string) => readFileSync(join(REFS, `${name}.md`), "utf8");

function expectAll(body: string, markers: (string | RegExp)[]): void {
  for (const marker of markers) {
    if (typeof marker === "string") expect(body).toContain(marker);
    else expect(body).toMatch(marker);
  }
}

test.concurrent("plan emits a machine-readable steps block", () => {
  expect(ref("plan")).toContain("Steps (machine-readable)");
});

test.concurrent("plan-review asserts every step has a runnable check and rejects empty steps", () => {
  expectAll(ref("plan-review"), ["non-empty runnable", "empty steps"]);
});

test.concurrent("execution reads status and records each step via run <plan_doc> --step <id>", () => {
  // The engine requires the plan-doc positional arg, so the doc must show it.
  expectAll(ref("execution"), ['plan-ledger.js" status', "run <plan_doc>", "--step <id>"]);
});

test.concurrent("spec documents the **AC-<n>:** id convention", () => {
  expect(ref("spec")).toContain("AC-<n>");
});

test.concurrent("spec authors delivery-critical evidence and impact sections", () => {
  expectAll(ref("spec"), [
    /^## Failure modes and edge cases$/m,
    /^## Acceptance evidence$/m,
    /^## Documentation impact$/m,
  ]);
});

test.concurrent("spec-review validates authored evidence and non-blocking questions", () => {
  expectAll(ref("spec-review"), [
    "Failure modes and edge cases",
    "Acceptance evidence",
    "Documentation impact",
    "observable real-data",
    "non-blocking",
  ]);
});

test.concurrent("shared plan reference documents optional ac_refs/depends_on/input fields", () => {
  expectAll(ref("ledger"), ["ac_refs", "depends_on", "input"]);
});

test.concurrent("shared plan reference uses the supported final verification command", () => {
  expect(ref("ledger")).toContain('plan-ledger.js" run <plan_doc> --verify');
});

test.concurrent("plan is evidence-first and keeps detailed steps only in the ledger", () => {
  expectAll(ref("plan"), [
    "Evidence first",
    "Mechanical work",
    "sole detailed step list",
    "ledger.md",
  ]);
  expect(existsSync(join(REFS, "ledger.md"))).toBe(true);
});

test.concurrent("plan delegates the full ledger schema to its shared reference", () => {
  expect(ref("plan")).not.toContain("each item has non-empty");
  expect(ref("plan")).toContain("canonical ledger shape");
});

test.concurrent("plan-review asserts ac_refs resolve via pl_check_ac_refs", () => {
  expectAll(ref("plan-review"), ["ac_refs", "pl_check_ac_refs"]);
});

test.concurrent("plan-review checks delivery-ready evidence and coverage", () => {
  expectAll(ref("plan-review"), [
    "AC-to-step coverage",
    "real input",
    "Path declarations",
    "PR-delivery readiness",
  ]);
});

test.concurrent("test is an execution-time method with a behavior-to-evidence map", () => {
  const body = ref("test");
  expectAll(body, [
    "reusable execution-time method",
    "Behavior-to-evidence map",
    "mock-substitute",
    "happy-path-only",
  ]);
  expect(body).not.toContain("mocks/");
});

test.concurrent("workflow docs show execution-time real-data tests", () => {
  for (const doc of ["README.md", join("docs", "toolu", "README.md")]) {
    expect(readFileSync(join(ROOT, doc), "utf8")).toMatch(/execution with real-data tests/i);
  }
});

test.concurrent("execution reads AC coverage from status", () => {
  expect(ref("execution")).toContain("AC-coverage");
});
