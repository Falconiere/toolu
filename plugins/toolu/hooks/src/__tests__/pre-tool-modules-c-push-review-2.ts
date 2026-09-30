/**
 * push-review cases, second half (#262): detached and linked worktrees, the
 * delivery modes and waivers, the `push_check` telemetry sites, and the
 * diff-sha parity test. Telemetry lines are recorded by the case runner.
 */
import { join } from "node:path";
import { bashFixture } from "@toolu/conformance/harness/fixtures";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import {
  BASE,
  commitFile,
  diffShaIn,
  FEATURE,
  gitIn,
  type GateCase,
} from "./pre-tool-modules-c-cases.ts";
import {
  addDetachedWorktree,
  addWorktree,
  ask,
  bare,
  pushReviewMode,
  shipped,
  strict,
  worktreeOf,
  writeState,
  writeWaiver,
} from "./pre-tool-modules-c-push-review-lib.ts";

const PUSH = "git push";
const REQUIRED = "Code review required";

const pushIn = (sb: Sandbox) => bashFixture(`git -C ${worktreeOf(sb)} push`);
const pushHead = (sb: Sandbox) =>
  bashFixture(`git -C ${worktreeOf(sb)} push origin HEAD:${FEATURE}`);

/** A detached worktree whose branch's review state is clean. */
function detachedClean(sb: Sandbox): void {
  addDetachedWorktree(sb);
  writeState(worktreeOf(sb), {}, FEATURE);
}

/** A linked worktree of `feat/example`, the main checkout parked on a diverging `other`. */
function divergedWorktree(sb: Sandbox): void {
  addWorktree(sb);
  gitIn(sb.project, ["checkout", "-q", "-b", "other", BASE]);
  commitFile(sb, "other.txt", "other");
}

/** A waiver for the current diff of the feature branch. */
function waived(sb: Sandbox): void {
  writeWaiver(sb.project, diffShaIn(sb.project));
}

export const PUSH_REVIEW_CASES_2: GateCase[] = [
  strict({
    name: "push-review: reviewed_files missing one changed file is denied naming the path",
    setup: (sb) => {
      commitFile(sb, "second.txt", "second");
      writeState(sb.project, { files: ["feature.txt"] });
    },
    command: PUSH,
    expect: "deny",
    has: ["reviewed_files does not match", "second.txt"],
  }),
  strict({
    name: "push-review: reviewed_files with the full changed-file list is allowed",
    setup: (sb) => {
      commitFile(sb, "second.txt", "second");
      writeState(sb.project, { files: ["feature.txt", "second.txt"] });
    },
    command: PUSH,
    expect: "silent",
  }),
  strict({
    name: "push-review: reviewed_files with an extra path not in the diff is denied naming it",
    setup: (sb) => writeState(sb.project, { files: ["feature.txt", "nonexistent.txt"] }),
    command: PUSH,
    expect: "deny",
    has: ["reviewed_files does not match", "nonexistent.txt"],
  }),
  strict({
    name: "push-review: detached worktree pushing HEAD:<branch> is judged as that branch and passes with a clean state",
    setup: detachedClean,
    fixture: pushHead,
    expect: "silent",
  }),
  strict({
    name: "push-review: detached worktree pushing HEAD:<branch> WITHOUT a state file is denied for the missing review, not as detached",
    setup: addDetachedWorktree,
    fixture: pushHead,
    expect: "deny",
    has: [REQUIRED, "feat_example"],
    lacks: ["detached HEAD"],
  }),
  strict({
    name: "push-review: detached worktree pushing with no refspec is still denied as detached, naming the fix",
    setup: detachedClean,
    fixture: pushIn,
    expect: "deny",
    has: ["detached HEAD", "HEAD:<branch>"],
  }),
  strict({
    name: "push-review: git -C <worktree> push is gated on the worktree, not the cwd",
    setup: addWorktree,
    fixture: pushIn,
    expect: "deny",
    has: ["Code review required before push"],
  }),
  strict({
    name: "push-review: a state file in the main checkout does not authorize a worktree push",
    setup: (sb) => {
      addWorktree(sb);
      const wt = worktreeOf(sb);
      writeState(sb.project, { sha: diffShaIn(wt), files: ["feature.txt"] }, FEATURE);
    },
    fixture: pushIn,
    expect: "deny",
    has: ["/wt/.claude/tmp/push-review/feat_example.json"],
  }),
  strict({
    name: "push-review: worktree push allowed once the worktree's own state is clean",
    setup: (sb) => {
      divergedWorktree(sb);
      writeState(worktreeOf(sb));
    },
    fixture: pushIn,
    expect: "silent",
  }),
  strict({
    name: "push-review: worktree push still denies on open findings",
    setup: (sb) => {
      addWorktree(sb);
      writeState(worktreeOf(sb), { count: 2 });
    },
    fixture: pushIn,
    expect: "deny",
    has: ["open findings"],
  }),
  strict({
    name: "push-review: $CLAUDE_PROJECT_DIR no longer decides where state is read",
    setup: (sb) => {
      divergedWorktree(sb);
      writeState(worktreeOf(sb));
    },
    env: (sb) => ({ CLAUDE_PROJECT_DIR: join(sb.root, "elsewhere") }),
    fixture: pushIn,
    expect: "silent",
  }),
  shipped({
    name: "push-review: default preset advises instead of denying",
    config: { version: 1 },
    command: PUSH,
    expect: "advisory",
    has: [REQUIRED],
  }),
  shipped({
    name: "push-review: default advise does not record a pending waiver",
    config: { version: 1 },
    command: PUSH,
    expect: "advisory",
  }),
  ask({
    name: "push-review: ask mode keeps the reviewer instructions",
    command: PUSH,
    expect: "ask",
    has: ["Code review required before push", "remembered until the diff changes"],
  }),
  ask({
    name: "push-review: asking records a pending waiver for the current diff",
    command: PUSH,
    expect: "ask",
  }),
  ask({
    name: "push-review: a promoted waiver lets the same diff through silently",
    setup: (sb) => {
      waived(sb);
    },
    command: PUSH,
    expect: "silent",
  }),
  ask({
    name: "push-review: a new commit invalidates the waiver and asks again",
    setup: (sb) => {
      waived(sb);
      commitFile(sb, "more.txt", "more");
    },
    command: PUSH,
    expect: "ask",
  }),
  ask({
    name: "push-review: a waiver does not cover a different branch",
    setup: (sb) => {
      waived(sb);
      gitIn(sb.project, ["checkout", "-q", "-b", "feat/other"]);
      commitFile(sb, "other.txt", "other");
    },
    command: PUSH,
    expect: "ask",
  }),
  shipped({
    name: "push-review: a clean review still passes under the default preset",
    config: { version: 1 },
    setup: (sb) => {
      writeState(sb.project);
    },
    command: PUSH,
    expect: "silent",
  }),
  shipped({
    name: "push-review: open findings advise rather than deny under the default preset",
    config: { version: 1 },
    setup: (sb) => {
      writeState(sb.project, { count: 2 });
    },
    command: PUSH,
    expect: "advisory",
    has: ["open findings"],
  }),
  shipped({
    name: "push-review: per-gate off emits nothing at all",
    config: pushReviewMode("off"),
    command: PUSH,
    expect: "silent",
  }),
  shipped({
    name: "push-review: advise mode reports without deciding",
    config: pushReviewMode("advise"),
    command: PUSH,
    expect: "advisory",
    has: [REQUIRED],
  }),
  ask({
    name: "push-review: ask degrades to advise on codex",
    host: "codex",
    command: PUSH,
    expect: "advisory",
    has: [REQUIRED],
  }),
  bare({
    name: "push-review: an empty diff advises under the default preset",
    config: { version: 1 },
    command: PUSH,
    expect: "advisory",
    has: ["is empty"],
  }),
  ask({
    name: "push-review: telemetry records the waived push",
    setup: (sb) => {
      waived(sb);
    },
    command: PUSH,
    expect: "silent",
  }),
];
