/**
 * plan-ledger cases (#262): every `@test` of the deleted plan-ledger.bats and
 * plan-ledger-acblock.bats, the `ac_coverage` test of telemetry-sites.bats and
 * the plan-ledger parity test of diff-sha.bats. The bats suites pinned the
 * `strict` preset (blocking delivery) and wrote ledgers with `jq`; the cases do
 * the same in `setup`, at the default ledger path of the claude host. The
 * dispatcher drops a module's stderr when it exits 0, so an advisory the bats
 * suite read on stderr is `silent` here. The AC cases hand-author the ledger the
 * real checker (`plan-ledger.sh run <plan> --verify`) writes.
 */
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { commitFile, group, writeIn, type GateCase } from "./pre-tool-modules-c-cases.ts";
import { cfg, LEDGER, put, seed, step } from "./pre-tool-modules-c-plan-ledger-kit.ts";
import { PLAN_LEDGER_AC_CASES } from "./pre-tool-modules-c-plan-ledger-2.ts";

const PUSH = "git push origin feat/example";
const RUN = "run: bash";

const enriched = {
  last_run: "2026-06-17T12:00:00Z",
  evidence_tail: "",
  started_at: null,
  activity: null,
};
const retry = {
  attempt: 1,
  exit_code: 1,
  diff_sha: "old",
  evidence_tail: "boom",
  at: "2026-06-16T00:00:00Z",
};

const pl = group({ config: cfg(), command: PUSH });

/** A verified-contract ledger with one green step; `verified` is the key's value. */
function verifiedSetup(status: string, verified: string | null) {
  return (sb: Sandbox) => {
    const sha = seed(sb, false);
    const title = "one";
    put(sb, [step("s1", title, status, sha)], {
      verified_sha: verified === "current" ? sha : verified,
    });
  };
}

const BATS_CASES: GateCase[] = [
  pl({
    name: "plan-ledger: non-push command (git status) allows silently",
    setup: (sb) => void seed(sb),
    command: "git status",
    expect: "silent",
  }),
  pl({
    name: "plan-ledger: non-Bash tool allows silently",
    setup: (sb) => void seed(sb),
    fixture: () => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "Grep",
      toolInput: { command: "git push" },
    }),
    expect: "silent",
  }),
  pl({
    name: "plan-ledger: no ledger + only .txt diff allows silently",
    setup: (sb) => {
      seed(sb, false);
      commitFile(sb, "notes.txt", "notes");
    },
    expect: "silent",
  }),
  pl({
    // bats read the stderr advisory; the dispatcher drops it on an allow.
    name: "plan-ledger: no ledger + code file allows (stderr advisory dropped)",
    setup: (sb) => void seed(sb),
    expect: "silent",
  }),
  pl({
    name: "plan-ledger: ledger with a red step denies and names it",
    setup: (sb) => {
      const sha = seed(sb);
      put(sb, [step("s1", "first thing", "red", sha, { exit_code: 1 })]);
    },
    expect: "deny",
    has: ["s1", "red", RUN],
  }),
  pl({
    name: "plan-ledger: all fresh-green allows",
    setup: (sb) => {
      const sha = seed(sb);
      put(sb, [step("s1", "first", "green", sha), step("s2", "second", "green", sha)]);
    },
    expect: "silent",
  }),
  pl({
    // A red ledger under another branch slug; the current branch has none.
    name: "plan-ledger: other-branch ledger does not affect current push",
    setup: (sb) => {
      const sha = seed(sb);
      put(
        sb,
        [step("s1", "x", "red", sha, { exit_code: 1 })],
        { branch: "other/branch" },
        1,
        ".claude/tmp/plan-ledger/other_branch.json",
      );
    },
    expect: "silent",
  }),
  pl({
    name: "plan-ledger: green-but-stale step denies and marks stale",
    setup: (sb) => {
      seed(sb);
      put(sb, [step("s1", "first", "green", "stale000")]);
    },
    expect: "deny",
    has: ["s1", "stale"],
  }),
  pl({
    name: "plan-ledger: empty-steps ledger is a no-op allow",
    setup: (sb) => {
      seed(sb);
      put(sb, []);
    },
    expect: "silent",
  }),
  pl({
    name: "plan-ledger: version!=1 denies (fail closed)",
    setup: (sb) => {
      const sha = seed(sb);
      put(sb, [step("s1", "first", "green", sha)], {}, 2);
    },
    expect: "deny",
    has: ["schema mismatch"],
  }),
  pl({
    name: "plan-ledger: a running step denies and is named as a blocker",
    setup: (sb) => {
      const sha = seed(sb);
      put(sb, [
        step("s1", "first", "green", sha, { started_at: null, activity: null }),
        step("s2", "second", "running", null, {
          started_at: "2026-06-17T12:00:00Z",
          activity: "checking s2",
          exit_code: null,
        }),
      ]);
    },
    expect: "deny",
    has: ["s2", "running"],
  }),
  pl({
    name: "plan-ledger: version-1 all-fresh-green ledger allows",
    setup: (sb) => {
      const sha = seed(sb);
      const extra = { started_at: null, activity: null };
      put(sb, [
        step("s1", "first", "green", sha, extra),
        step("s2", "second", "green", sha, extra),
      ]);
    },
    expect: "silent",
  }),
  pl({
    name: "plan-ledger: unparseable ledger denies (fail closed)",
    setup: (sb) => {
      seed(sb);
      writeIn(sb.project, LEDGER, "not json {{{");
    },
    expect: "deny",
    has: ["unparseable ledger"],
  }),
  pl({
    name: "plan-ledger: enriched v1 all-fresh-green ledger allows",
    setup: (sb) => {
      const sha = seed(sb);
      put(sb, [
        step("s1", "first", "green", sha, {
          ...enriched,
          ac_refs: ["AC-1"],
          depends_on: ["s0"],
          input: "fixture",
          retries: [retry],
        }),
        step("s2", "second", "green", sha, {
          ...enriched,
          ac_refs: [],
          depends_on: [],
          input: null,
          retries: [],
        }),
      ]);
    },
    expect: "silent",
  }),
  pl({
    name: "plan-ledger: enriched v1 ledger with a red step denies",
    setup: (sb) => {
      const sha = seed(sb);
      put(sb, [
        step("s1", "first", "green", sha, {
          ...enriched,
          ac_refs: ["AC-1"],
          depends_on: [],
          input: null,
          retries: [],
        }),
        step("s2", "second", "red", sha, {
          ...enriched,
          check: "false",
          exit_code: 1,
          evidence_tail: "nope",
          ac_refs: ["AC-2"],
          depends_on: ["s1"],
          input: "fixture",
          retries: [retry],
        }),
      ]);
    },
    expect: "deny",
    has: ["s2", "red"],
  }),
  pl({
    name: "plan-ledger: all green but never verified asks for a verify run",
    setup: verifiedSetup("green", null),
    command: "git push",
    expect: "deny",
    has: ["--verify"],
  }),
  pl({
    name: "plan-ledger: a verified ledger at the current diff allows",
    setup: verifiedSetup("green", "current"),
    command: "git push",
    expect: "silent",
  }),
  pl({
    name: "plan-ledger: a verified ledger from an older diff asks again",
    setup: verifiedSetup("green", "stale-sha-from-an-earlier-diff"),
    command: "git push",
    expect: "deny",
    has: ["--verify"],
  }),
  pl({
    name: "plan-ledger: a red step is still reported before any verify talk",
    setup: verifiedSetup("red", null),
    command: "git push",
    expect: "deny",
    has: ["s1"],
    lacks: ["--verify"],
  }),
  pl({
    name: "plan-ledger: a legacy ledger without the key keeps the old contract",
    setup: (sb) => {
      const sha = seed(sb, false);
      put(sb, [step("s1", "one", "green", sha)]);
    },
    command: "git push",
    expect: "silent",
  }),
];

export const PLAN_LEDGER_CASES: GateCase[] = [...BATS_CASES, ...PLAN_LEDGER_AC_CASES];
