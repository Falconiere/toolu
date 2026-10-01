/** Contract for the private execution reference and delivery readiness. */

import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const EXECUTION = join(import.meta.dir, "..", "delivery-flow", "references", "execution.md");
const VERIFY = 'plan-ledger.js" run <plan_doc> --verify';
const text = () => readFileSync(EXECUTION, "utf8");

/** 1-based number of the first line matching `pattern`, or 0 when none does. */
function lineOf(body: string, pattern: RegExp): number {
  return body.split("\n").findIndex((line) => pattern.test(line)) + 1;
}

test.concurrent("execution verifies every ledger step against the final branch diff", () => {
  expect(text()).toContain(VERIFY);
  expect(text()).toMatch(/AC coverage/i);
});

test.concurrent("execution requires per-step real-data evidence and documentation sync", () => {
  expect(text()).toMatch(/per-step.*real-data/i);
  expect(text()).toMatch(/Docs in sync/i);
});

test.concurrent("execution owns the v2 review state and ready unified verdict", () => {
  expect(text()).toMatch(/toolu-review.*version: 2|version: 2.*toolu-review/i);
  expect(text()).toContain('verdict.js" status');
  expect(text()).toMatch(/overall: ready/i);
});

test.concurrent("execution attests the committed branch before delivery", () => {
  const body = text();
  expect(body.split(VERIFY).length - 1).toBe(1);
  const order = [
    lineOf(body, /commit the scoped changes/i),
    lineOf(body, /plan-ledger\.js" run <plan_doc> --verify/),
    lineOf(body, /toolu-review:review/),
    lineOf(body, /verdict\.js" status/),
    lineOf(body, /push the non-default feature branch/i),
  ];
  expect(order.every((line) => line > 0)).toBe(true);
  expect(order).toEqual(order.toSorted((a, b) => a - b));
  expect(new Set(order).size).toBe(order.length);
});

test.concurrent("execution only delivers with authorization and all delivery prerequisites", () => {
  const body = text();
  for (const pattern of [
    /delivery authorization/i,
    /GitHub API authentication/i,
    /non-default branch/i,
    /pr-babysit.*installed/i,
  ]) {
    expect(body).toMatch(pattern);
  }
});

test.concurrent("execution follows the required brainstorm phase", () => {
  expect(text()).toMatch(/brainstorm.*already run/i);
});

test.concurrent("execution commits pushes creates or finds a default-branch PR then invokes babysit", () => {
  const body = text();
  for (const pattern of [
    /commit.*scoped changes/i,
    /push/i,
    /locate.*or create.*pull request|create.*or locate.*pull request/i,
    /repository default branch/i,
    /verify.*PR.*number.*head\/base|verify.*head\/base.*branches/i,
  ]) {
    expect(body).toMatch(pattern);
  }
  expect(body).toContain("pr-babysit:babysit");
});

test.concurrent("execution has no execution-review or terminal-test handoff", () => {
  expect(text()).not.toMatch(/execution-review/i);
  expect(text()).not.toMatch(/then to `test` for the final pass/i);
});

test.concurrent("execution uses private routing and host-neutral delivery invocations", () => {
  const body = text();
  expect(body).toContain("[model-routing.md](model-routing.md)");
  expect(body).toContain("[host-mapping.md](host-mapping.md)");
  expect(body).not.toContain("plugins/toolu/skills/orchestrator/references/model-routing.md");
  expect(body).not.toContain("$toolu-review:review");
  expect(body).not.toContain("$pr-babysit:babysit");
});
