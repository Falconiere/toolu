/** Shared configs, state writers and worktree setup for the push-review cases (#262). */
import { join } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import {
  BASE,
  changedFiles,
  diffShaIn,
  featureRepo,
  FEATURE,
  gitIn,
  group,
  type CaseInput,
  type GateCase,
  headBranch,
  slugOf,
  writeIn,
} from "./pre-tool-modules-c-cases.ts";

export const STRICT = { version: 1, gates: { preset: "strict" } };
export const ASK = { version: 1, gates: { pushReview: { mode: "ask" } } };

export function pushReviewMode(mode: string): object {
  return { version: 1, gates: { pushReview: { mode } } };
}

/** A case builder on the bats `setup_sandbox` repo; the case's own `setup` runs after it. */
function onFeatureRepo(config?: object): (c: CaseInput) => GateCase {
  const build = group(config === undefined ? {} : { config });
  return (c) =>
    build({
      ...c,
      setup: (sb) => {
        featureRepo(sb);
        c.setup?.(sb);
      },
    });
}

/** The bats suites' `use_strict_preset` default. */
export const strict = onFeatureRepo(STRICT);
/** The shipped default preset (advise). */
export const shipped = onFeatureRepo();
/** The opt-in `ask` mode. */
export const ask = onFeatureRepo(ASK);

/** An empty-diff branch: `feat/example` cut from base with no commit. */
export const bare = group({
  setup: (sb) => gitIn(sb.project, ["checkout", "-q", "-b", FEATURE]),
});

/** The state file's path under the repo root, per the claude host layout. */
export function statePath(branch: string, ext = "json"): string {
  return `.claude/tmp/push-review/${slugOf(branch)}.${ext}`;
}

export type StateSpec = {
  version?: number;
  sha?: string;
  count?: number;
  /** `null` leaves `review_round` out. */
  round?: number | null;
  reviewers?: string[];
  /** `null` leaves `reviewed_files` out. */
  files?: string[] | null;
};

/**
 * The bats `write_state`: a v2 state for `branch` (default the checked-out one)
 * of the repo at `dir`, `reviewed_files` from the real diff unless given.
 */
export function writeState(dir: string, spec: StateSpec = {}, branch = headBranch(dir)): void {
  const files = spec.files === undefined ? changedFiles(dir) : spec.files;
  const state = {
    version: spec.version ?? 2,
    branch,
    diff_sha: spec.sha ?? diffShaIn(dir),
    base_branch: BASE,
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: spec.reviewers ?? ["code-review"],
    findings_count: spec.count ?? 0,
    ...(spec.round === null ? {} : { review_round: spec.round ?? 1 }),
    findings: [],
    ...(files === null ? {} : { reviewed_files: files }),
  };
  writeIn(dir, statePath(branch), JSON.stringify(state));
}

/** A state file with exactly this body. */
export function writeRawState(dir: string, body: string, branch = headBranch(dir)): void {
  writeIn(dir, statePath(branch), body);
}

/** A promoted waiver for `sha`, the file `push_waiver_promote` writes. */
export function writeWaiver(dir: string, sha: string, branch = headBranch(dir)): void {
  const waiver = {
    version: 1,
    branch: slugOf(branch),
    diff_sha: sha,
    base_branch: BASE,
    reason_code: "no-state",
    waived_at: "2026-07-28T00:00:00Z",
  };
  writeIn(dir, statePath(branch, "waiver.json"), JSON.stringify(waiver));
}

/** Where the extra worktree lives. */
export function worktreeOf(sb: Sandbox): string {
  return join(sb.root, "wt");
}

/** The bats `setup_worktree`: the main checkout back on base, `feat/example` in a worktree. */
export function addWorktree(sb: Sandbox): void {
  gitIn(sb.project, ["checkout", "-q", BASE]);
  gitIn(sb.project, ["worktree", "add", "-q", worktreeOf(sb), FEATURE]);
}

/** A detached worktree at HEAD (pr-babysit's `git worktree add --detach`). */
export function addDetachedWorktree(sb: Sandbox): void {
  gitIn(sb.project, ["worktree", "add", "-q", "--detach", worktreeOf(sb), "HEAD"]);
}
