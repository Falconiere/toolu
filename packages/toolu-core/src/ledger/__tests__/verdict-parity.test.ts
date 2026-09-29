/**
 * AC-6 (#256): `verdictMain` vs `verdict.sh` on twin real repos. It covers
 * every `verdict.bats` case (quality fail and skip, all ready, plan advise,
 * round-cap escalate, empty diff, schema, schema-v1, version 3, docs
 * attestation through `CLAUDE_PROJECT_DIR` and through the cwd, not a repo)
 * and the remaining gate branches. Each compares the JSON report, the status
 * table and the exit code.
 */
import { describe, expect, test } from "bun:test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { diffSha } from "../../state/diff-sha.ts";
import { verdictMain } from "../verdict.ts";
import { ACCEPTED_REVIEWERS } from "../review-state.ts";
import { LIB } from "./ledger-parity-helpers.ts";
import { commit, planDoc, step, twin, type Action, type Scenario } from "./ledger-scenario.ts";

const cfg = (sb: Sandbox): string => {
  const dir = join(sb.root, "cfg");
  mkdirSync(dir, { recursive: true });
  return dir;
};
const verdict = (mode: string, extra: Partial<Action> = {}): Action => ({
  cli: "verdict.sh",
  argv: () => [mode],
  ...extra,
});
const both = (extra: Partial<Action> = {}): Action[] => [
  verdict("json", extra),
  verdict("status", extra),
];

/** Every action gets an empty user config dir, so the machine's config never leaks in. */
function withCfg(scenario: Scenario): Scenario {
  return {
    ...scenario,
    actions: scenario.actions.map((a) => ({
      ...a,
      env: (sb: Sandbox) => ({
        TOOLU_CONFIG_DIR: cfg(sb),
        ...(typeof a.env === "function" ? a.env(sb) : a.env),
      }),
    })),
  };
}

const sha = (sb: Sandbox, cwd = sb.project): string => diffSha(cwd, "main") ?? "";
const wtDir = (sb: Sandbox): string => join(sb.root, "wt");

function reviewState(sb: Sandbox, fields: Record<string, unknown>): void {
  const branch = sb.git("rev-parse", "--abbrev-ref", "HEAD").trim();
  const slug = branch.replaceAll("/", "_").replace(/[^A-Za-z0-9_-]/g, "");
  const files = sb.git("diff", "--name-only", "main...HEAD").split("\n").filter(Boolean);
  const body = {
    version: 2,
    branch,
    diff_sha: sha(sb),
    base_branch: "main",
    reviewed_at: "2026-07-30T00:00:00Z",
    reviewers: ["code-review"],
    findings_count: 0,
    findings: [],
    review_round: 1,
    reviewed_files: files,
    ...fields,
  };
  sb.write(`.claude/tmp/push-review/${slug}.json`, body);
}

function addCode(sb: Sandbox, files: Record<string, string> = { "lib/foo.sh": "echo hi\n" }): void {
  for (const [rel, body] of Object.entries(files)) sb.write(rel, body);
  commit(sb, "code");
}

/** The real bash plan-ledger CLI writes the ledger: the setup is identical in both twins. */
function ledgerRun(sb: Sandbox, steps: unknown[], header = ""): void {
  sb.write("plan.md", planDoc(steps, header));
  const res = Bun.spawnSync(["bash", join(LIB, "plan-ledger.sh"), "run", sb.path("plan.md")], {
    cwd: sb.project,
    env: {
      PATH: process.env.PATH ?? "",
      HOME: sb.home,
      PUSH_REVIEW_BASE: "main",
      TOOLU_HOST_OVERRIDE: "claude",
    },
  });
  if (res.exitCode === 2) throw new Error(res.stderr.toString());
}

const SCENARIOS: Record<string, Scenario> = {
  "recorded quality failure: blocked": {
    setup: (sb) =>
      sb.write(".claude/tmp/quality-gate-status.json", {
        status: "failing",
        reason: "bad ts\nline",
      }),
    actions: both(),
  },
  "quality file passing / reason missing": {
    setup: (sb) => sb.write(".claude/tmp/quality-gate-status.json", { status: "passing" }),
    actions: [
      ...both(),
      {
        ...verdict("json"),
        before: (s) =>
          s.write(".claude/tmp/quality-gate-status.json", '{"status":"failing","reason":null}'),
      },
      {
        ...verdict("json"),
        before: (s) => s.write(".claude/tmp/quality-gate-status.json", "garbage"),
      },
    ],
  },
  "everything green: ready": {
    setup: (sb) => {
      addCode(sb, { "lib/foo.sh": "echo hi\n", "docs/foo.md": "# doc\n" });
      reviewState(sb, {});
      ledgerRun(sb, [step("s1", "true")]);
    },
    actions: both(),
  },
  "linked worktree: quality skip, docs attestation via CLAUDE_PROJECT_DIR": {
    setup: (sb) => {
      sb.git("worktree", "add", "-q", "-b", "feat/docs", join(sb.root, "wt"), "main");
      const wt = join(sb.root, "wt");
      mkdirSync(join(wt, "lib"), { recursive: true });
      writeFileSync(join(wt, "lib/foo.sh"), "echo hi\n");
      sb.git("-C", wt, "add", "lib/foo.sh");
      sb.git("-C", wt, "commit", "-qm", "add foo.sh");
      const pd = join(sb.root, "claude-project-dir");
      mkdirSync(join(pd, ".claude/tmp/docs-sync"), { recursive: true });
      writeFileSync(
        join(pd, ".claude/tmp/docs-sync/feat_docs.json"),
        JSON.stringify({ version: 1, diff_sha: sha(sb, wt), decision: "not-needed" }),
      );
    },
    actions: [
      ...both({
        cwd: wtDir,
        env: (sb) => ({ CLAUDE_PROJECT_DIR: join(sb.root, "claude-project-dir") }),
      }),
      ...both({ cwd: wtDir }),
    ],
  },
  "plan-less code diff: advise, still ready": {
    setup: (sb) => {
      addCode(sb);
      reviewState(sb, {});
    },
    actions: both(),
  },
  "review round 6: escalate": {
    setup: (sb) => {
      addCode(sb, { "script.sh": "echo\n" });
      reviewState(sb, { review_round: 6.5 });
    },
    actions: both(),
  },
  "empty diff": {
    setup: (sb) => sb.git("checkout", "-q", "-b", "feat/empty", "main"),
    actions: both(),
  },
  "review state variants": {
    setup: (sb) => addCode(sb, { "script.sh": "echo\n" }),
    actions: [
      verdict("json"),
      {
        ...verdict("json"),
        before: (s) => s.write(".claude/tmp/push-review/feat_x.json", "not json"),
      },
      { ...verdict("json"), before: (s) => s.write(".claude/tmp/push-review/feat_x.json", "null") },
      { ...verdict("json"), before: (s) => reviewState(s, { version: 1 }) },
      { ...verdict("json"), before: (s) => reviewState(s, { version: 3 }) },
      { ...verdict("json"), before: (s) => reviewState(s, { version: "2", diff_sha: "" }) },
      { ...verdict("json"), before: (s) => reviewState(s, { reviewers: ["nobody"] }) },
      { ...verdict("json"), before: (s) => reviewState(s, { reviewers: "code-review" }) },
      { ...verdict("json"), before: (s) => reviewState(s, { reviewers: 5 }) },
      { ...verdict("json"), before: (s) => reviewState(s, { diff_sha: "stale" }) },
      { ...verdict("json"), before: (s) => reviewState(s, { reviewed_files: ["other"] }) },
      { ...verdict("json"), before: (s) => reviewState(s, { reviewed_files: "script.sh" }) },
      {
        ...verdict("json"),
        before: (s) => reviewState(s, { findings_count: 2, review_round: "x" }),
      },
      { ...verdict("json"), before: (s) => reviewState(s, { review_round: 5 }) },
      {
        ...verdict("json"),
        before: (s) => reviewState(s, { reviewed_files: ["script.sh", "script.sh"] }),
      },
      { ...verdict("json"), before: (s) => s.write(".claude/tmp/push-review/feat_x.json", "[1]") },
      { ...verdict("json"), before: (s) => reviewState(s, { reviewed_files: ["", "script.sh"] }) },
      { ...verdict("json"), before: (s) => reviewState(s, { reviewed_files: ["script.sh", ""] }) },
      { ...verdict("json"), before: (s) => reviewState(s, { reviewed_files: [["script.sh"]] }) },
    ],
  },
  "plan ledger variants": {
    setup: (sb) => {
      sb.write(
        "spec.md",
        "**Status:** Approved\n\n## Acceptance criteria\n\n- **AC-1:** a\n- **AC-2:** b\n",
      );
      addCode(sb);
      reviewState(sb, {});
      ledgerRun(
        sb,
        [step("s1", "true", { ac_refs: ["AC-1"] }), step("s2", "false")],
        "**Spec:** spec.md",
      );
    },
    actions: [
      verdict("json"),
      { ...verdict("json"), before: (s) => addCode(s, { "lib/bar.ts": "x\n" }) },
      {
        ...verdict("json"),
        before: (s) => s.write(".claude/tmp/plan-ledger/feat_x.json", '{"version":2}'),
      },
      {
        ...verdict("json"),
        before: (s) => s.write(".claude/tmp/plan-ledger/feat_x.json", "garbage"),
      },
      {
        ...verdict("json"),
        before: (s) => s.write(".claude/tmp/plan-ledger/feat_x.json", '{"version":1,"steps":[]}'),
      },
      {
        ...verdict("json"),
        before: (s) =>
          s.write(
            ".claude/tmp/plan-ledger/feat_x.json",
            '{"version":1,"steps":[{"id":"a","status":"green"}]}',
          ),
      },
      {
        ...verdict("json"),
        before: (s) =>
          s.write(
            ".claude/tmp/plan-ledger/feat_x.json",
            '{"version":"1","summary":{"total":2},"steps":"ab"}',
          ),
      },
      {
        ...verdict("json"),
        before: (s) =>
          s.write(
            ".claude/tmp/plan-ledger/feat_x.json",
            '{"version":1,"summary":{"total":3},"steps":[{"id":"a","status":"red"},{"id":7,"status":"red"},{"id":"c"}]}',
          ),
      },
      { ...verdict("json"), env: (sb) => ({ LEDGER_DIR: join(sb.root, "nowhere") }) },
    ],
  },
  "docs gate modes and bases": {
    setup: (sb) => {
      addCode(sb, { "src/a.ts": "x\n" });
      reviewState(sb, {});
    },
    actions: [
      verdict("json"),
      {
        ...verdict("json"),
        before: (s) =>
          s.writeConfig("claude", "project", { version: 1, docsSync: { mode: "block" } }),
      },
      {
        ...verdict("json"),
        before: (s) =>
          s.writeConfig("claude", "project", { version: 1, docsSync: { mode: "off" } }),
      },
      {
        ...verdict("json"),
        before: (s) =>
          s.writeConfig("claude", "project", { version: 1, docsSync: { mode: "loud" } }),
      },
      { ...verdict("json"), env: { DOCS_SYNC_BASE: "nope" } },
      { ...verdict("json"), env: (sb) => ({ DOCS_SYNC_STATE_DIR: join(sb.root, "att") }) },
      { ...verdict("status"), env: { PUSH_REVIEW_BASE: "feat/x" } },
    ],
  },
  "docs attestation via the cwd fallback": {
    setup: (sb) => {
      addCode(sb);
      sb.write(".claude/tmp/docs-sync/feat_x.json", { version: 1, diff_sha: sha(sb) });
    },
    actions: both(),
  },
  "on the base branch": { setup: (sb) => sb.git("checkout", "-q", "main"), actions: both() },
  "detached HEAD": { setup: (sb) => sb.git("checkout", "-q", "--detach"), actions: both() },
  "codex host": {
    setup: (sb) => {
      addCode(sb);
      sb.write(".codex/tmp/quality-gate-status.json", { status: "failing", reason: "codex" });
    },
    actions: both({ env: { TOOLU_HOST_OVERRIDE: "codex" } }),
  },
  "not a git repository": { noRepo: true, actions: both() },
  "bad usage": { actions: [verdict(""), verdict("xml")] },
};

describe("verdictMain vs verdict.sh", () => {
  for (const [name, scenario] of Object.entries(SCENARIOS)) {
    test.concurrent(
      name,
      async () => {
        const [bash, port] = await twin(withCfg(scenario), verdictMain);
        expect(port).toEqual(bash);
      },
      60_000,
    );
  }
});

/** The native push-review gate imports ACCEPTED_REVIEWERS; verdict.sh keeps its own copy. */
test("ACCEPTED_REVIEWERS equals verdict.sh's ACCEPTED_REVIEWERS literal", () => {
  const verdictSrc = readFileSync(join(LIB, "verdict.sh"), "utf8");
  expect(verdictSrc).toContain(`ACCEPTED_REVIEWERS='${JSON.stringify(ACCEPTED_REVIEWERS)}'`);
});
