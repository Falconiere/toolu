/**
 * Plan-doc parsing (ported from plan-parse.bats): steps-block extraction,
 * validation, optional-field backfill, and `**Issue:**` key extraction, on the
 * real plan docs under fixtures/ and small docs written per test.
 */
import { afterEach, beforeEach, expect, test } from "bun:test";
import { CliExit } from "@toolu/core/cli";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { docField, issueKey, parseSteps } from "../jira/plan-parse.ts";
import { FIXTURES } from "./harness.ts";

const OK = join(FIXTURES, "plan-ok.md");
const BAD = join(FIXTURES, "plan-bad-steps.md");

let sandbox = "";
beforeEach(() => {
  sandbox = mkdtempSync(join(tmpdir(), "jira-parse-"));
});
afterEach(() => rmSync(sandbox, { recursive: true, force: true }));

function doc(name: string, content: string): string {
  const path = join(sandbox, name);
  writeFileSync(path, content);
  return path;
}

function failure(run: () => unknown): CliExit {
  try {
    run();
  } catch (error) {
    if (error instanceof CliExit) return error;
    throw error;
  }
  throw new Error("expected a CliExit");
}

test("parse_steps: extracts the fenced block under the exact heading", () => {
  const steps = parseSteps(OK);
  expect(steps.map((step) => step.id)).toEqual(["comment-pr-link", "transition-done"]);
});

test("parse_steps: prose after the closing fence is not captured", () => {
  expect(JSON.stringify(parseSteps(OK))).not.toContain("must not be captured");
});

test("parse_steps: backfills activity to null, preserves an authored one", () => {
  const [first, second] = parseSteps(OK);
  expect(first?.activity).toBeNull();
  expect(second?.activity).toBe("transitioning");
});

test("parse_steps: a step missing check is rejected", () => {
  const exit = failure(() => parseSteps(BAD));
  expect(exit.code).toBe(1);
  expect(exit.message).toContain("not a non-empty array of {id,title,check}");
});

test("parse_steps: missing doc errors", () => {
  const missing = join(sandbox, "nope.md");
  expect(failure(() => parseSteps(missing)).message).toBe(
    `jira plan: plan doc not found: ${missing}`,
  );
});

test("parse_steps: doc without the heading is rejected", () => {
  const bare = doc("bare.md", "# no steps here\n");
  expect(failure(() => parseSteps(bare)).message).toBe(
    `jira plan: no '## Steps (machine-readable)' json block in ${bare}`,
  );
});

test("issue_key: reads the packed **Issue:** header field", () => {
  expect(issueKey(OK)).toBe("ABC-123");
});

test("issue_key: rejects a doc with no Issue header", () => {
  const noKey = doc("nokey.md", "# t\n\n**Date:** 2026-07-09\n");
  expect(failure(() => issueKey(noKey)).message).toBe(
    `jira plan: doc is missing a valid '**Issue:** <KEY>' header: ${noKey}`,
  );
});

test("issue_key: rejects a malformed key (path traversal cannot reach the filename)", () => {
  const evil = doc("evil.md", "# t\n\n**Issue:** ../../etc/passwd   **Topic:** x\n");
  expect(failure(() => issueKey(evil)).code).toBe(1);
});

test("doc_field: a trailing field runs to end of line", () => {
  expect(docField(OK, "Topic")).toBe("move the ticket to Done and record the PR link");
  expect(docField(OK, "Date")).toBe("2026-07-09");
  expect(docField(OK, "Absent")).toBe("");
});
