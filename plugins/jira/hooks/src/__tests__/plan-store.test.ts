/**
 * The ledger store (ported from plan-store.bats): paths inside a real git
 * repository, the build/merge from the real plan-ok.md fixture, summary
 * recompute, atomic write and read. base_branch "" and diff_sha null are
 * asserted by name: they stop the dashboard marking Jira cards stale.
 */
import { afterEach, beforeEach, expect, test } from "bun:test";
import { CliExit } from "@toolu/core/cli";
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseSteps } from "../jira/plan-parse.ts";
import {
  buildLedger,
  docPath,
  evidence,
  type Ledger,
  ledgerPath,
  readLedger,
  setStep,
  writeLedger,
} from "../jira/plan-store.ts";
import { FIXTURES } from "./harness.ts";

const DOC = join(FIXTURES, "plan-ok.md");
const CLAUDE = {};

let repo = "";
beforeEach(() => {
  repo = realpathSync(mkdtempSync(join(tmpdir(), "jira-store-")));
  spawnSync("git", ["init", "-q", repo]);
});
afterEach(() => rmSync(repo, { recursive: true, force: true }));

function failure(run: () => unknown): CliExit {
  try {
    run();
  } catch (error) {
    if (error instanceof CliExit) return error;
    throw error;
  }
  throw new Error("expected a CliExit");
}

function build(prev?: Record<string, unknown>): Ledger {
  return buildLedger("ABC-123", DOC, parseSteps(DOC), prev);
}

function step(ledger: Ledger, id: string): Record<string, unknown> | undefined {
  return ledger.steps.find((item) => item["id"] === id);
}

test("ledger_path: jira-<KEY>.json under the repo root, never the branch slug", () => {
  expect(ledgerPath("ABC-123", CLAUDE, repo)).toBe(
    join(repo, ".claude/tmp/plan-ledger/jira-ABC-123.json"),
  );
});

test("ledger_path: resolves the repo root from a subdirectory", () => {
  const sub = join(repo, "a", "b");
  mkdirSync(sub, { recursive: true });
  expect(ledgerPath("ABC-123", CLAUDE, sub)).toBe(
    join(repo, ".claude/tmp/plan-ledger/jira-ABC-123.json"),
  );
});

test("ledger_path: refuses an empty key", () => {
  expect(failure(() => ledgerPath("", CLAUDE, repo)).message).toBe(
    "jira plan: not a valid issue key: ''",
  );
});

test("ledger_path: refuses keys that would escape the ledger dir", () => {
  for (const bad of [
    "../../etc/passwd",
    "ABC-1/../../x",
    "/abs/path",
    "ABC-1;rm -rf .",
    "ABC",
    "-123",
    "ABC-",
    "ABC-1 2",
  ]) {
    const exit = failure(() => ledgerPath(bad, CLAUDE, repo));
    expect(exit.message).toContain("not a valid issue key");
  }
});

test("doc_path: refuses keys that would escape the plans dir", () => {
  expect(failure(() => docPath("../../../evil", CLAUDE, repo)).message).toContain(
    "not a valid issue key",
  );
});

test("ledger_path: accepts the real Jira key shapes", () => {
  for (const ok of ["ABC-123", "A-1", "PROJ_X-9999"]) {
    expect(ledgerPath(ok, CLAUDE, repo)).toEndWith(`/plan-ledger/jira-${ok}.json`);
  }
});

test("doc_path: <repo>/.claude/tmp/jira/plans/<KEY>.md", () => {
  expect(docPath("ABC-123", CLAUDE, repo)).toBe(join(repo, ".claude/tmp/jira/plans/ABC-123.md"));
});

test("paths: Codex host uses <repo>/.codex/tmp without affecting Claude defaults", () => {
  const codex = { TOOLU_HOST_OVERRIDE: "codex" };
  expect(ledgerPath("ABC-123", codex, repo)).toBe(
    join(repo, ".codex/tmp/plan-ledger/jira-ABC-123.json"),
  );
  expect(docPath("ABC-123", codex, repo)).toBe(join(repo, ".codex/tmp/jira/plans/ABC-123.md"));
  expect(docPath("ABC-123", { PLUGIN_ROOT: "/p" }, repo)).toStartWith(join(repo, ".codex"));
  expect(
    docPath("ABC-123", { PLUGIN_ROOT: "/p", TOOLU_HOST_OVERRIDE: "claude" }, repo),
  ).toStartWith(join(repo, ".claude"));
});

test("build: emits schema version 1 with branch jira-<KEY>", () => {
  expect(build()).toMatchObject({ version: 1, branch: "jira-ABC-123", plan_doc: DOC });
});

test("build: base_branch is exactly the empty string (disables dashboard staleness)", () => {
  expect(build().base_branch).toBe("");
});

test("build: every step carries diff_sha null", () => {
  expect(build().steps.every((item) => item["diff_sha"] === null)).toBe(true);
});

test("build: fresh steps are pending with the full step field set, in bash's key order", () => {
  const ledger = build();
  expect(Object.keys(ledger)).toEqual([
    "version",
    "branch",
    "base_branch",
    "plan_doc",
    "updated_at",
    "summary",
    "next",
    "steps",
  ]);
  expect(ledger.steps).toHaveLength(2);
  expect(ledger.steps.every((item) => item["status"] === "pending")).toBe(true);
  expect(Object.keys(ledger.steps[0] ?? {})).toEqual([
    "id",
    "title",
    "check",
    "status",
    "started_at",
    "activity",
    "exit_code",
    "diff_sha",
    "last_run",
    "evidence_tail",
  ]);
  expect(ledger.updated_at).toMatch(/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/);
});

test("build: summary agrees with steps; stale is 0 and fresh_green tracks green", () => {
  expect(build().summary).toEqual({
    total: 2,
    green: 0,
    red: 0,
    pending: 2,
    running: 0,
    stale: 0,
    fresh_green: 0,
    retried: 0,
  });
});

test("build: next is the first non-green step", () => {
  expect(build().next).toBe("comment-pr-link");
});

test("build: carries persisted status forward by step id, authored title wins", () => {
  const ledger = build({
    steps: [
      {
        id: "comment-pr-link",
        status: "green",
        exit_code: 0,
        last_run: "2026-07-09T10:00:00Z",
        title: "stale title",
      },
    ],
  });
  expect(step(ledger, "comment-pr-link")).toMatchObject({
    status: "green",
    exit_code: 0,
    last_run: "2026-07-09T10:00:00Z",
    title: "Comment the PR link on ABC-123",
  });
  expect(step(ledger, "transition-done")?.["status"]).toBe("pending");
  expect(ledger.summary.green).toBe(1);
  expect(ledger.next).toBe("transition-done");
});

test("build: all steps green => next is null", () => {
  const ledger = build({
    steps: [
      { id: "comment-pr-link", status: "green" },
      { id: "transition-done", status: "green" },
    ],
  });
  expect(ledger.next).toBeNull();
  expect(ledger.summary).toMatchObject({ green: 2, fresh_green: 2 });
});

test("set_step: merges fields into one step and recomputes the summary", () => {
  const ledger = setStep(build(), "transition-done", { status: "red", exit_code: 1 });
  expect(step(ledger, "transition-done")).toMatchObject({ status: "red", exit_code: 1 });
  expect(ledger.summary).toMatchObject({ red: 1, pending: 1 });
});

test("write: is atomic, creates the dir, and leaves no temp file", () => {
  const target = join(repo, ".claude/tmp/plan-ledger/jira-ABC-123.json");
  const ledger = build();
  writeLedger(target, ledger);
  const content = readFileSync(target, "utf8");
  expect(content).toBe(`${JSON.stringify(ledger, null, 2)}\n`);
  expect(readdirSync(join(repo, ".claude/tmp/plan-ledger"))).toEqual(["jira-ABC-123.json"]);
});

test("write: an uncreatable ledger dir fails and writes nothing", () => {
  writeFileSync(join(repo, ".claude"), "a file, not a directory");
  const target = join(repo, ".claude/tmp/plan-ledger/jira-ABC-123.json");
  const exit = failure(() => writeLedger(target, build()));
  expect(exit.message).toBe(
    `jira plan: cannot create ledger dir: ${join(repo, ".claude/tmp/plan-ledger")}`,
  );
});

test("read: absent, empty, corrupt or null ledger is treated as absent", () => {
  expect(readLedger(join(repo, "nope.json"))).toBeUndefined();
  for (const content of ["", "not json", "null"]) {
    writeFileSync(join(repo, "bad.json"), content);
    expect(readLedger(join(repo, "bad.json"))).toBeUndefined();
  }
});

test("read: round-trips what write persisted", () => {
  const target = join(repo, ".claude/tmp/plan-ledger/jira-ABC-123.json");
  writeLedger(target, build());
  expect(readLedger(target)).toMatchObject({ branch: "jira-ABC-123", base_branch: "" });
});

test("evidence: keeps the last 10 lines", () => {
  const out = Array.from({ length: 12 }, (_, at) => `l${at + 1}`).join("\n");
  const tail = evidence(out);
  expect(tail).toContain("l12");
  expect(tail.split("\n")).toEqual(["l3", "l4", "l5", "l6", "l7", "l8", "l9", "l10", "l11", "l12"]);
});

test("evidence: caps at 2000 bytes; a cut character becomes U+FFFD", () => {
  const tail = evidence(`${"a".repeat(1999)}é tail`);
  expect(tail).toBe(`${"a".repeat(1999)}\uFFFD`);
});
