/**
 * push-review cases, third part (#262): the `push_check` telemetry sites of
 * telemetry-sites.bats and the push-review parity test of diff-sha.bats. The
 * telemetry lines themselves are recorded by the case runner.
 */
import { mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import {
  BASE,
  changedFiles,
  diffShaIn,
  gitIn,
  headBranch,
  type GateCase,
} from "./pre-tool-modules-c-cases.ts";
import {
  bare,
  shipped,
  strict,
  writeRawState,
  writeState,
} from "./pre-tool-modules-c-push-review-lib.ts";

const PUSH = "git push";

/** A clean v2 state whose `review_round` / `reviewers` are exactly these JSON values. */
function rawClean(sb: Sandbox, round: unknown, reviewers: unknown = ["code-review"]): void {
  const state = {
    version: 2,
    branch: headBranch(sb.project),
    diff_sha: diffShaIn(sb.project),
    base_branch: BASE,
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers,
    findings_count: 0,
    review_round: round,
    findings: [],
    reviewed_files: changedFiles(sb.project),
  };
  writeRawState(sb.project, JSON.stringify(state));
}
const REQUIRED = "Code review required";

/** Corrupt the object store after the refs exist, so `git diff` fails for real. */
function corruptObjects(sb: Sandbox): void {
  const objects = join(sb.project, ".git", "objects");
  rmSync(objects, { recursive: true, force: true });
  mkdirSync(objects);
}

export const PUSH_REVIEW_CASES_3: GateCase[] = [
  strict({
    name: "push-review: push_check no state file denies with reason_code=no-state, round=null",
    command: PUSH,
    expect: "deny",
    has: [REQUIRED],
  }),
  shipped({
    name: "push-review: push_check clean push allows with reason_code=pass and the state's round",
    setup: (sb) => writeState(sb.project, { round: 3 }),
    command: PUSH,
    expect: "silent",
  }),
  shipped({
    name: "push-review: push_check open findings denies with reason_code=findings and the round",
    setup: (sb) => writeState(sb.project, { count: 2, round: 1 }),
    command: PUSH,
    expect: "advisory",
    has: ["open findings"],
  }),
  shipped({
    name: "push-review: push_check round-cap escalation denies with reason_code=round-cap",
    setup: (sb) => writeState(sb.project, { round: 6 }),
    command: PUSH,
    expect: "advisory",
    has: ["ESCALATE"],
  }),
  strict({
    name: "push-review: push_check base branch missing locally denies with reason_code=base-missing, round=null",
    env: () => ({ PUSH_REVIEW_BASE: "nonexistent-base" }),
    command: PUSH,
    expect: "deny",
    has: ["nonexistent-base", "git fetch"],
  }),
  shipped({
    name: "push-review: push_check git diff failure (corrupted objects) allows with reason_code=diff-failed, round=null",
    setup: corruptObjects,
    command: PUSH,
    expect: "silent",
  }),
  bare({
    name: "push-review: push_check empty diff denies with reason_code=empty-diff, round=null",
    command: PUSH,
    expect: "advisory",
    has: ["is empty"],
  }),
  shipped({
    name: "push-review: push_check stale diff denies with reason_code=stale-diff and the round",
    setup: (sb) =>
      writeState(sb.project, { sha: "stale0000000000000000000000000000000000", round: 2 }),
    command: PUSH,
    expect: "advisory",
    has: ["diff changed since review"],
  }),
  shipped({
    name: "push-review: push_check no accepted reviewer denies with reason_code=reviewer and the round",
    setup: (sb) =>
      writeState(sb.project, { reviewers: ["simplify"], round: 4, files: ["feature.txt"] }),
    command: PUSH,
    expect: "advisory",
    has: ["no accepted reviewer"],
  }),
  shipped({
    name: "push-review: push_check corrupted state file denies with reason_code=schema, round=null",
    setup: (sb) => writeRawState(sb.project, "not json\n"),
    command: PUSH,
    expect: "advisory",
    has: ["corrupted"],
  }),
  shipped({
    name: "push-review: push_check reviewed_files short of the diff denies with reason_code=file-coverage and the round",
    setup: (sb) => writeState(sb.project, { round: 2, files: [] }),
    command: PUSH,
    expect: "advisory",
    has: ["reviewed_files does not match"],
  }),
  strict({
    name: "push-review: parity (push-review.sh) empty diff against base still denied with sentinel reason",
    setup: (sb) => {
      gitIn(sb.project, ["checkout", "-q", BASE]);
      gitIn(sb.project, ["checkout", "-q", "-b", "feat/empty"]);
    },
    command: PUSH,
    expect: "deny",
    has: ["empty"],
  }),
  // Exact-port boundaries, beyond the bats suites.
  strict({
    name: "push-review: a string round of 010 is octal 8 in bash arithmetic and escalates",
    setup: (sb) => rawClean(sb, "010"),
    command: PUSH,
    expect: "deny",
    has: ["ESCALATE: review loop hit 010 rounds"],
  }),
  strict({
    name: "push-review: a string round of 09 is not octal, so it never trips the cap",
    setup: (sb) => rawClean(sb, "09"),
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: a reviewers string is searched as text, as jq index does",
    setup: (sb) => rawClean(sb, 1, "notes from code-review-bot"),
    command: PUSH,
    expect: "silent",
  }),
];
