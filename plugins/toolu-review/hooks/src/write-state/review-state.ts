/**
 * The push-review v2 state file (#269). The review judgement belongs to the
 * caller; this does the deterministic bookkeeping the gate checks: repo root,
 * branch, base, `diff_sha`, slug, `reviewed_files` and `review_round`. Each
 * recipe comes from `@toolu/core`, the same code the TypeScript gates use, so
 * writer and gate cannot drift.
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { CliExit } from "@toolu/core/cli";
import { envValue, projectStateDir, type HostEnv } from "@toolu/core/host";
import {
  baseBranch,
  branchSlug,
  compareJqStrings,
  currentBranch,
  diffSha,
  hasGit,
  isoSeconds,
  toJqJson,
  writeAtomic,
} from "@toolu/core/state";
import { z } from "zod";
import { TOOL, type WriteStateArgs } from "./args.ts";

const EMPTY_BLOB_SHA = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";

function fail(message: string): never {
  throw new CliExit(1, `${TOOL}: ${message}`);
}

/** `git args` in `cwd`: stdout, or `undefined` when git fails. */
function git(cwd: string, args: readonly string[], env: HostEnv): string | undefined {
  const res = spawnSync("git", args, { cwd, env: { ...env }, encoding: "utf8" });
  return res.error === undefined && res.status === 0 ? res.stdout : undefined;
}

function repoRoot(repo: string | undefined, env: HostEnv, cwd: string): string {
  const top = git(cwd, ["-C", repo ?? ".", "rev-parse", "--show-toplevel"], env)?.trim();
  if (top === undefined || top === "") fail(`${repo ?? cwd} is not inside a git repo`);
  return top;
}

/** The checked-out branch; on a detached checkout the validated `--branch` the push targets. */
function targetBranch(root: string, requested: string | undefined, env: HostEnv): string {
  const branch = currentBranch(root, env);
  if (branch !== "" && branch !== "HEAD") {
    if (requested !== undefined && requested !== branch) {
      fail(
        `--branch '${requested}' does not match the checked-out branch '${branch}'; the gate keys the state file by the checked-out branch`,
      );
    }
    return branch;
  }
  if (requested === undefined) {
    fail(
      "not on a branch (detached HEAD?) — pass --branch <name> naming the branch the push targets (git push origin HEAD:<name>)",
    );
  }
  if (git(root, ["check-ref-format", "--branch", requested], env) === undefined) {
    fail(`--branch '${requested}' is not a valid branch name`);
  }
  const known = ["refs/heads", "refs/remotes/origin"].some(
    (prefix) =>
      git(root, ["show-ref", "--verify", "--quiet", `${prefix}/${requested}`], env) !== undefined,
  );
  if (!known) {
    fail(
      `unknown branch '${requested}' (no refs/heads/${requested} or refs/remotes/origin/${requested})`,
    );
  }
  return requested;
}

/** Sorted, unique, non-empty paths in UTF-8 byte order (jq `unique`). */
function sortedUnique(paths: readonly string[]): string[] {
  return [...new Set(paths.filter((path) => path !== ""))].toSorted(compareJqStrings);
}

function reviewedFiles(root: string, base: string, override: string | undefined, env: HostEnv) {
  if (override !== undefined) return sortedUnique(override.split(","));
  const names = git(root, ["-C", root, "diff", "--no-color", `${base}...HEAD`, "--name-only"], env);
  if (names === undefined) fail("failed to compute reviewed_files");
  return sortedUnique(names.split("\n"));
}

const PriorSchema = z.looseObject({
  diff_sha: z.unknown(),
  review_round: z.union([z.number(), z.string()]).optional().catch(undefined),
});

/** The previous round at the same `diff_sha`, else 0: a changed diff restarts the count. */
function previousRound(file: string, sha: string): number {
  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(file, "utf8"));
  } catch {
    return 0;
  }
  const prior = PriorSchema.safeParse(raw);
  if (!prior.success || prior.data.diff_sha !== sha) return 0;
  const text = String(prior.data.review_round ?? "");
  return /^[0-9]+$/.test(text) ? Number(text) : 0;
}

function parseJson(text: string): unknown {
  try {
    const value: unknown = JSON.parse(text);
    return value;
  } catch {
    return fail("jq failed (bad --reviewers/--findings/--reviewed-files JSON?)");
  }
}

/** Write the state file for `args` and return its path; failures throw CliExit. */
export function writeReviewState(args: WriteStateArgs, env: HostEnv, cwd: string): string {
  if (!hasGit(env)) throw new CliExit(2, `${TOOL}: git required`);
  const root = repoRoot(args.repo, env, cwd);
  const branch = targetBranch(root, args.branch, env);
  const base = envValue(env, "PUSH_REVIEW_BASE") ?? baseBranch(root, env);
  const sha = diffSha(root, base, { env }) ?? fail(`git diff ${base}...HEAD failed`);
  if (sha === EMPTY_BLOB_SHA) fail(`diff against ${base} is empty; nothing to review yet`);
  const files = reviewedFiles(root, base, args.reviewedFiles, env);
  const dir = envValue(env, "STATE_DIR") ?? projectStateDir("push-review", { env, root }) ?? root;
  const file = join(dir, `${branchSlug(branch)}.json`);
  try {
    mkdirSync(dir, { recursive: true });
  } catch {
    fail(`cannot create ${dir}`);
  }
  const doc = {
    version: 2,
    branch,
    diff_sha: sha,
    base_branch: base,
    reviewed_at: isoSeconds(new Date()),
    reviewers: parseJson(args.reviewers),
    findings_count: args.findingsCount,
    findings: parseJson(args.findings),
    review_round: previousRound(file, sha) + 1,
    reviewed_files: files,
  };
  if (!writeAtomic(file, `${toJqJson(doc, true)}\n`)) fail("atomic mv failed");
  return file;
}
