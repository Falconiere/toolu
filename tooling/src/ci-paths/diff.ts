/**
 * The diff a `changes` job classifies (#458): `pull_request` compares
 * base...head, `push` compares before..after, and anything else (including
 * `workflow_dispatch`) runs every group.
 */
import { spawnSync } from "node:child_process";
import { z } from "zod";
import type { ChangedFile } from "./classify.ts";
import { type CiPaths, matchesAny } from "./config.ts";

export type DiffRange = { kind: "range"; spec: string } | { kind: "all"; reason: string };

const Sha = z.string().regex(/^[0-9a-f]{40}$/);
const ZERO_SHA = /^0{40}$/;

const PullRequestEvent = z.looseObject({
  pull_request: z.looseObject({
    base: z.looseObject({ sha: Sha }),
    head: z.looseObject({ sha: Sha }),
  }),
});
const PushEvent = z.looseObject({ before: Sha, after: Sha });

/** The range `event` asks for, or why every group runs instead. */
export function resolveRange(eventName: string, event: unknown): DiffRange {
  if (eventName === "pull_request") {
    const parsed = PullRequestEvent.safeParse(event);
    if (!parsed.success)
      return { kind: "all", reason: "the pull_request event lacks base/head SHAs" };
    const { base, head } = parsed.data.pull_request;
    return { kind: "range", spec: `${base.sha}...${head.sha}` };
  }
  if (eventName === "push") {
    const parsed = PushEvent.safeParse(event);
    if (!parsed.success) return { kind: "all", reason: "the push event lacks before/after SHAs" };
    if (ZERO_SHA.test(parsed.data.before)) {
      return { kind: "all", reason: "the push has no previous commit" };
    }
    return { kind: "range", spec: `${parsed.data.before}..${parsed.data.after}` };
  }
  return { kind: "all", reason: `the ${eventName || "unknown"} event runs everything` };
}

export class DiffError extends Error {}

function git(repo: string, args: string[]): string {
  const res = spawnSync("git", ["-C", repo, ...args], { encoding: "utf8" });
  if (res.error !== undefined) throw new DiffError(`git ${args.join(" ")}: ${res.error.message}`);
  if (res.status !== 0) {
    throw new DiffError(`git ${args.join(" ")} exited ${String(res.status)}: ${res.stderr.trim()}`);
  }
  return res.stdout;
}

function changedLines(repo: string, spec: string, path: string): string[] {
  const patch = git(repo, ["diff", "-U0", "--no-renames", "--no-color", spec, "--", path]);
  return patch.split("\n").filter((line) => /^[+-]/.test(line) && !/^(?:\+\+\+|---) /.test(line));
}

/**
 * The files `spec` changes in `repo`. Diff lines are read only for
 * release-only paths, the one place classification looks at content.
 */
export function readDiff(repo: string, spec: string, config: CiPaths): ChangedFile[] {
  const names = git(repo, ["diff", "--name-only", "--no-renames", "-z", spec]);
  return names
    .split("\0")
    .filter((path) => path !== "")
    .map((path) => ({
      path,
      lines: matchesAny(config.releaseOnly.paths, path) ? changedLines(repo, spec, path) : [],
    }));
}
