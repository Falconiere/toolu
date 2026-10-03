/** finish-issue.ts against real git: snapshot before teardown, refusals leave the record alone.
 * Records carry no agent and no workspace_id, so teardown never reaches herdr or gh. */

import { expect, test } from "bun:test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import {
  acquireLease,
  patchLease,
  readResourceState,
  writeJsonAtomic,
} from "@toolu/core/resources";
import { bindWorktree } from "@toolu/core/resources/binding";
import { deleteBranch, finishRemovedWorktree } from "../finish-issue.ts";
import { readIssueRecord } from "../finish-state.ts";

const FINISH = join(import.meta.dir, "..", "finish-issue.ts");
const IDENTITY = {
  GIT_AUTHOR_NAME: "t",
  GIT_AUTHOR_EMAIL: "t@t",
  GIT_COMMITTER_NAME: "t",
  GIT_COMMITTER_EMAIL: "t@t",
};

async function git(cwd: string, ...args: string[]): Promise<string> {
  const res = await run(["git", "-C", cwd, ...args], { env: IDENTITY });
  if (res.exitCode !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr}`);
  return res.stdout.trim();
}

type Layout = { main: string; wt: string; state: string; record: string };

/** origin (bare) <- main clone, a linked worktree on feat/1-x, and its issue record. */
async function layout(sb: Sandbox): Promise<Layout> {
  const origin = join(sb.root, "origin.git");
  const main = join(sb.root, "main");
  const wt = join(sb.root, "wt");
  const state = join(sb.root, "state");
  await git(sb.root, "init", "-q", "--bare", "-b", "main", origin);
  await git(sb.root, "clone", "-q", origin, main);
  writeFileSync(join(main, "a.txt"), "one\n");
  await git(main, "add", "a.txt");
  await git(main, "commit", "-qm", "init");
  await git(main, "push", "-q", "origin", "main");
  await git(main, "worktree", "add", "-q", "-b", "feat/1-x", wt, "origin/main");
  const rec = {
    key: "k-1",
    repo: "o/r",
    branch: "feat/1-x",
    checkout: main,
    worktree: wt,
  };
  const record = join(state, "issues", "k-1.json");
  mkdirSync(dirname(record), { recursive: true });
  writeFileSync(record, `${JSON.stringify({ ...rec, stage: "running" }, null, 2)}\n`);
  return { main, wt, state, record };
}

const finish = (...args: string[]) => run(["bun", FINISH, ...args]);

const readRecord = (path: string) =>
  JSON.parse(readFileSync(path, "utf8")) as Record<string, unknown>;

test.concurrent("abandon snapshots uncommitted and untracked work and records the ref", async () => {
  using sb = createSandbox();
  const l = await layout(sb);
  writeFileSync(join(l.wt, "a.txt"), "two\n");
  writeFileSync(join(l.wt, "new.txt"), "new\n");
  const res = await finish(l.state, "k-1", "--abandon");
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain("WARN: no workspace_id on record k-1");
  const rec = readRecord(l.record);
  expect(rec.stage).toBe("cleanup-incomplete");
  expect(rec.wip_ref).toBe("refs/epic-wip/k-1");
  expect(rec.worktree_removed).toBe(false);
  expect(rec.branch_deleted).toBe(false);
  expect(rec.cleanup_attempt_at).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/);
  expect(rec.finished_at).toBeUndefined();
  expect(rec.leftover_files).toEqual([" M a.txt", "?? new.txt"]);
  expect(await git(l.main, "show", "refs/epic-wip/k-1:a.txt")).toBe("two");
  expect(await git(l.main, "show", "refs/epic-wip/k-1:new.txt")).toBe("new");
  expect(JSON.parse(res.stdout)).toEqual({
    key: "k-1",
    stage: "cleanup-incomplete",
    worktree_removed: false,
    branch_deleted: false,
    wip_ref: "refs/epic-wip/k-1",
    leftover_files: [" M a.txt", "?? new.txt"],
  });
});

test.concurrent("clean, pushed worktree records no wip ref", async () => {
  using sb = createSandbox();
  const l = await layout(sb);
  const res = await finish(l.state, "k-1", "--abandon");
  expect(res.exitCode).toBe(1);
  const rec = readRecord(l.record);
  expect(rec.wip_ref).toBeNull();
  expect(rec.leftover_files).toEqual([]);
  const ref = await run([
    "git",
    "-C",
    l.main,
    "rev-parse",
    "--verify",
    "--quiet",
    "refs/epic-wip/k-1",
  ]);
  expect(ref.exitCode).not.toBe(0);
});

test.concurrent("without --abandon a record with no PR is refused and left unchanged", async () => {
  using sb = createSandbox();
  const l = await layout(sb);
  const before = readFileSync(l.record, "utf8");
  const res = await finish(l.state, "k-1");
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain("no PR recorded for k-1; use --abandon");
  expect(readFileSync(l.record, "utf8")).toBe(before);
});

test.concurrent("a missing, malformed, or non-object record fails without writing", async () => {
  using sb = createSandbox();
  const state = join(sb.root, "state");
  const missing = await finish(state, "k-9", "--abandon");
  expect(missing.exitCode).toBe(1);
  expect(missing.stderr).toContain(`invalid or missing issue record`);
  const arrayRecord = join(state, "issues", "k-8.json");
  mkdirSync(dirname(arrayRecord), { recursive: true });
  writeFileSync(arrayRecord, "[1]\n");
  const notObject = await finish(state, "k-8", "--abandon");
  expect(notObject.exitCode).toBe(1);
  expect(notObject.stderr).toContain(`invalid or missing issue record ${arrayRecord}`);
  expect(readFileSync(arrayRecord, "utf8")).toBe("[1]\n");
  const badRecord = join(state, "issues", "k-7.json");
  writeFileSync(badRecord, '{"key":"k-7","worktree":4}\n');
  const malformed = await finish(state, "k-7", "--abandon");
  expect(malformed.exitCode).toBe(1);
  expect(malformed.stderr).toContain(`invalid issue record ${badRecord}: repo`);
  expect(readFileSync(badRecord, "utf8")).toBe('{"key":"k-7","worktree":4}\n');
});

test.concurrent("bad arguments fail without writing", async () => {
  using sb = createSandbox();
  const state = join(sb.root, "state");
  for (const args of [[state], [state, "k-9", "--force"], [state, "k-9", "--abandon", "x"]]) {
    const usage = await finish(...args);
    expect(usage.exitCode).toBe(2);
    expect(usage.stderr).toContain("usage: finish-issue.ts");
  }
});

test("all active lifecycle stages are valid finish records", () => {
  using sb = createSandbox();
  const path = join(sb.root, "record.json");
  for (const stage of ["awaiting_merge", "replacing", "cleaning"]) {
    writeJsonAtomic(path, {
      key: "k-1",
      repo: "o/r",
      branch: "feat/1-x",
      checkout: sb.root,
      worktree: sb.project,
      stage,
    });
    expect(readIssueRecord(path, "k-1").stage).toBe(stage);
  }
});

test("a legacy bound worktree adopts and persists a cleanup fence", async () => {
  using sb = createSandbox();
  const l = await layout(sb);
  const root = join(sb.root, "resources");
  await writeJsonAtomic(join(root, "policy.json"), {
    maxAgents: 1,
    maxJobs: 1,
  });
  const lease = await acquireLease(root, {
    type: "agent",
    key: "k-1",
    stateDir: l.state,
    host: "codex",
    worktree: l.wt,
  });
  await patchLease(root, lease.token, { stage: "running" });
  bindWorktree(root, l.wt, "k-1", l.state);

  const result = await finish(l.state, "k-1", "--abandon");
  expect(result.exitCode).toBe(1);
  expect(readRecord(l.record)).toMatchObject({
    resource_root: root,
    lease_token: lease.token,
    stage: "cleanup-incomplete",
  });
  expect(readResourceState(root).leases[0]).toMatchObject({
    token: lease.token,
    worktree: l.wt,
    stage: "cleaning",
  });
});

test("cleanup retries branch deletion after the workspace was already removed", async () => {
  using sb = createSandbox();
  const l = await layout(sb);
  const root = join(sb.root, "resources");
  const blocker = join(sb.root, "branch-owner");
  const resource = await acquireLease(root, {
    type: "agent",
    key: "k-1",
    stateDir: l.state,
    host: "codex",
    worktree: l.wt,
  });
  await patchLease(root, resource.token, { stage: "running" });
  bindWorktree(root, l.wt, "k-1", l.state);
  writeJsonAtomic(l.record, {
    ...readRecord(l.record),
    resource_root: root,
    lease_token: resource.token,
  });
  await git(l.main, "worktree", "remove", "--force", l.wt);
  await git(l.main, "worktree", "add", "-q", blocker, "feat/1-x");

  const first = await finishRemovedWorktree(
    l.record,
    readIssueRecord(l.record, "k-1"),
    null,
    [],
    false,
  );
  expect(first.branchDeleted).toBe(false);
  expect(readRecord(l.record)).toMatchObject({
    stage: "cleanup-incomplete",
    worktree_removed: true,
    branch_deleted: false,
  });
  expect(readResourceState(root).leases).toHaveLength(1);
  await git(l.main, "worktree", "remove", "--force", blocker);
  await git(l.main, "branch", "-D", "feat/1-x");

  expect(await deleteBranch(readIssueRecord(l.record, "k-1"), true, false)).toBe(true);
});

test("an incomplete terminal record retries cleanup without releasing ownership", async () => {
  using sb = createSandbox();
  const l = await layout(sb);
  const root = join(sb.root, "resources");
  const lease = await acquireLease(root, {
    type: "agent",
    key: "k-1",
    stateDir: l.state,
    host: "codex",
    worktree: l.wt,
  });
  await patchLease(root, lease.token, { stage: "running" });
  bindWorktree(root, l.wt, "k-1", l.state);
  writeJsonAtomic(l.record, {
    ...readRecord(l.record),
    stage: "abandoned",
    worktree_removed: false,
    branch_deleted: false,
    resource_root: root,
    lease_token: lease.token,
  });

  const result = await finish(l.state, "k-1", "--abandon");

  expect(result.exitCode).toBe(1);
  expect(readRecord(l.record)).toMatchObject({
    stage: "cleanup-incomplete",
    worktree_removed: false,
    branch_deleted: false,
  });
  expect(readResourceState(root).leases).toEqual([
    expect.objectContaining({ token: lease.token, stage: "cleaning" }),
  ]);
});
