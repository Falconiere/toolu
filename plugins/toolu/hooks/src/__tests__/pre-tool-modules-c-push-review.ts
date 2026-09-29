/**
 * push-review cases (#262): every `@test` of the deleted push-review.bats, the
 * `push-review push_check:` tests of telemetry-sites.bats and the push-review
 * parity test of diff-sha.bats. The bats suite pinned the strict preset, so a
 * failed check reads as a deny; the base branch is `main`, not `development`.
 * Skipped as no longer meaningful: "empty slug falls back to _default" (pure
 * shell, drives no gate) and "dispatcher picks up the module by glob" (the
 * bundle registers its modules natively).
 */
import { join } from "node:path";
import {
  commitFile,
  diffShaIn,
  gitIn,
  writeIn,
  BASE,
  type GateCase,
} from "./pre-tool-modules-c-cases.ts";
import { PUSH_REVIEW_CASES_2 } from "./pre-tool-modules-c-push-review-2.ts";
import { PUSH_REVIEW_CASES_3 } from "./pre-tool-modules-c-push-review-3.ts";
import {
  shipped,
  strict,
  writeRawState,
  writeState,
} from "./pre-tool-modules-c-push-review-lib.ts";

const PUSH = "git push";

export const PUSH_REVIEW_CASES: GateCase[] = [
  strict({
    name: "push-review: non-Bash tool exits silently",
    fixture: (sb) => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "Read",
      toolInput: { file_path: join(sb.project, "base.txt"), command: PUSH },
    }),
    expect: "silent",
  }),
  strict({
    name: "push-review: Bash but not git push exits silently",
    command: "ls -la",
    expect: "silent",
  }),
  strict({
    name: "push-review: git push inside heredoc body is ignored",
    command: 'git commit -m "$(cat <<EOF\nabout git push\nEOF\n)"',
    expect: "advisory",
    has: ["BEFORE COMMITTING"],
    lacks: ["Code review required"],
  }),
  strict({
    name: "push-review: branch slug strips slashes",
    setup: (sb) => {
      gitIn(sb.project, ["checkout", "-q", "-b", "feat/x/y"]);
      commitFile(sb, "a.txt", "a");
    },
    command: PUSH,
    expect: "deny",
    has: ["feat_x_y.json"],
  }),
  strict({
    name: "push-review: diff SHA is stable for unchanged content",
    setup: (sb) => {
      writeState(sb.project, { sha: diffShaIn(sb.project) });
    },
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: diff SHA changes when content changes",
    setup: (sb) => {
      writeState(sb.project);
      writeIn(sb.project, "feature.txt", "feature\nmore\n");
      gitIn(sb.project, ["commit", "-q", "-am", "more"]);
    },
    command: PUSH,
    expect: "deny",
    has: ["diff changed since review"],
  }),
  strict({
    name: "push-review: diff SHA survives commit --amend with identical content",
    setup: (sb) => {
      writeState(sb.project);
      gitIn(sb.project, ["commit", "-q", "--amend", "--no-edit"]);
    },
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: git push with no state file is denied",
    command: PUSH,
    expect: "deny",
    has: ["Code review required", "atomically write"],
  }),
  strict({
    name: "push-review: git push with matching SHA and zero findings is allowed",
    setup: (sb) => writeState(sb.project),
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: git push with matching SHA and open findings is denied",
    setup: (sb) => writeState(sb.project, { count: 3 }),
    command: PUSH,
    expect: "deny",
    has: ["open findings"],
  }),
  strict({
    name: "push-review: git push with stale SHA is denied",
    setup: (sb) => writeState(sb.project, { sha: "stale_sha_value_0000000000000000000000" }),
    command: PUSH,
    expect: "deny",
    has: ["diff changed since review"],
  }),
  strict({
    name: "push-review: corrupted state file is denied",
    setup: (sb) => writeRawState(sb.project, "not json\n"),
    command: PUSH,
    expect: "deny",
    has: ["corrupted"],
  }),
  strict({
    name: "push-review: state file missing required keys is denied",
    setup: (sb) => writeRawState(sb.project, '{"version": 2}\n'),
    command: PUSH,
    expect: "deny",
    has: ["corrupted"],
  }),
  strict({
    name: "push-review: state file with wrong version is denied",
    setup: (sb) =>
      writeRawState(sb.project, JSON.stringify({ version: 3, diff_sha: "x", findings_count: 0 })),
    command: PUSH,
    expect: "deny",
    has: ["corrupted"],
  }),
  strict({
    name: "push-review: state file with schema v1 is denied with the dedicated upgrade message",
    setup: (sb) => writeState(sb.project, { version: 1, files: null }),
    command: PUSH,
    expect: "deny",
    has: [
      "push-review state is schema v1; harness v2 requires reviewed_files — re-run the review to regenerate the state file",
    ],
  }),
  strict({
    name: "push-review: missing base branch is denied with fetch hint",
    setup: (sb) => gitIn(sb.project, ["branch", "-q", "-D", BASE]),
    env: () => ({ PUSH_REVIEW_BASE: BASE }),
    command: PUSH,
    expect: "deny",
    has: ["base branch", "git fetch"],
  }),
  strict({
    name: "push-review: detached HEAD is denied",
    setup: (sb) =>
      gitIn(sb.project, ["checkout", "-q", gitIn(sb.project, ["rev-parse", "HEAD"]).trim()]),
    command: PUSH,
    expect: "deny",
    has: ["detached HEAD"],
  }),
  strict({
    name: "push-review: denial reason instructs agent to use atomic write",
    command: PUSH,
    expect: "deny",
    has: ["atomic"],
  }),
  strict({
    name: "push-review: empty diff against base is denied with sentinel reason (even with matching state file)",
    setup: (sb) => {
      gitIn(sb.project, ["checkout", "-q", BASE]);
      gitIn(sb.project, ["checkout", "-q", "-b", "feat/empty"]);
      writeState(sb.project, { sha: "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391" });
    },
    command: PUSH,
    expect: "deny",
    has: ["diff against", "empty"],
  }),
  strict({
    name: "push-review: pushing the base branch itself is ALLOWED (no state file required)",
    setup: (sb) => gitIn(sb.project, ["checkout", "-q", BASE]),
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: non-empty diff with matching SHA + zero findings still ALLOWED",
    setup: (sb) => writeState(sb.project),
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: state file with no accepted reviewer is denied",
    setup: (sb) => writeState(sb.project, { reviewers: ["simplify"], round: null }),
    command: PUSH,
    expect: "deny",
    has: ["no accepted reviewer"],
  }),
  strict({
    name: "push-review: the built-in code-review reviewer alone is allowed (agnostic baseline)",
    setup: (sb) => writeState(sb.project),
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: toolu-review:review reviewer alone satisfies the accepted set",
    setup: (sb) => writeState(sb.project, { reviewers: ["toolu-review:review"] }),
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: security-review reviewer alone satisfies the accepted set",
    setup: (sb) => writeState(sb.project, { reviewers: ["security-review"] }),
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: non-accepted reviewer alone is denied",
    setup: (sb) => writeState(sb.project, { reviewers: ["extra-pass"] }),
    command: PUSH,
    expect: "deny",
    has: ["no accepted reviewer"],
  }),
  strict({
    name: "push-review: extra reviewers beyond the accepted set still pass (superset)",
    setup: (sb) => writeState(sb.project, { reviewers: ["extra-pass", "code-review"] }),
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: review_round above MAX_ROUNDS triggers escalation deny",
    setup: (sb) => writeState(sb.project, { round: 6 }),
    command: PUSH,
    expect: "deny",
    has: ["ESCALATE", "Escalation stop"],
  }),
  strict({
    name: "push-review: review_round at MAX_ROUNDS still allowed",
    setup: (sb) => writeState(sb.project, { round: 5 }),
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: missing review_round defaults to 1 (backward compat)",
    setup: (sb) =>
      writeState(sb.project, { reviewers: ["extra-pass", "code-review"], round: null }),
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: full loop — state rewritten for the fixed diff is allowed",
    setup: (sb) => {
      writeState(sb.project, { count: 2 });
      writeIn(sb.project, "feature.txt", "feature\nfix\n");
      gitIn(sb.project, ["commit", "-q", "-am", "fix finding"]);
      writeState(sb.project);
    },
    command: PUSH,
    expect: "silent",
  }),
  shipped({
    name: "push-review: no state file advises at the shipped preset",
    command: PUSH,
    expect: "advisory",
    has: ["Code review required before push"],
  }),
  strict({
    name: "push-review: no state file denies under strict",
    command: PUSH,
    expect: "deny",
    has: ["Code review required before push"],
  }),
  ...PUSH_REVIEW_CASES_2,
  ...PUSH_REVIEW_CASES_3,
];
