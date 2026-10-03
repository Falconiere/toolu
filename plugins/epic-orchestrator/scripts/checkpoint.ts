/** Snapshot a worker's worktree so no progress is lost to a crash, a usage
 * limit, a host switch, or a teardown.
 *
 * The snapshot is a commit of the full working tree (tracked, staged, and
 * untracked-but-not-ignored files) on top of HEAD, built with a retained
 * independent index so the worker's own index and files are never touched. It
 * is stored at `refs/epic-wip/<key>` in the shared repository, with a reflog,
 * so it outlives the worktree. Restore with:
 * `git checkout -b recover refs/epic-wip/<key>`. */

import { createHash, randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, realpathSync, rmSync } from "node:fs";
import { isAbsolute, join, resolve } from "node:path";
import { runCommand } from "../hooks/dist/epic-runtime.js";
import { acquireLock, writeJsonAtomic } from "../hooks/dist/epic-runtime.js";
import { readJson } from "./common.ts";

export type Snapshot = {
  key: string;
  ref: string;
  sha: string | null;
  dirty: boolean;
  unpushed: number;
  changed: boolean;
  skipped?: string;
};

export type SnapshotOptions = {
  env?: NodeJS.ProcessEnv;
  commandTimeoutMs?: number;
  lockTimeoutMs?: number;
};

type GitResult = { out: string; ok: boolean };
type CheckpointMeta =
  | { version: 1; worktree: string; head: string }
  | { version: 2; worktree: string; head: string; index: string };
type CheckpointStorage = { path: string; worktree: string };

const DEFAULT_COMMAND_TIMEOUT_MS = 30_000;
const DEFAULT_LOCK_TIMEOUT_MS = 1_000;
const MAX_GIT_OUTPUT_BYTES = 64 * 1024;

async function git(
  cwd: string,
  args: string[],
  env: NodeJS.ProcessEnv,
  timeoutMs: number,
): Promise<GitResult> {
  const result = await runCommand(["git", "-C", cwd, ...args], {
    env,
    timeoutMs,
    maxOutputBytes: MAX_GIT_OUTPUT_BYTES,
  });
  return {
    out: result.stdout.trim(),
    ok: result.exitCode === 0 && !result.timedOut && !result.cancelled && !result.truncated,
  };
}

const IDENTITY = {
  GIT_AUTHOR_NAME: "epic-orchestrator",
  GIT_AUTHOR_EMAIL: "epic-orchestrator@localhost",
  GIT_COMMITTER_NAME: "epic-orchestrator",
  GIT_COMMITTER_EMAIL: "epic-orchestrator@localhost",
};

function positiveFinite(value: number | undefined, fallback: number, label: string): number {
  const selected = value ?? fallback;
  if (!Number.isFinite(selected) || selected <= 0) {
    throw new Error(`${label} must be a positive finite number`);
  }
  return selected;
}

async function checkpointDirectory(
  worktree: string,
  env: NodeJS.ProcessEnv,
  timeoutMs: number,
): Promise<{ path: string; worktree: string } | null> {
  let canonical: string;
  try {
    canonical = realpathSync(worktree);
  } catch {
    return null;
  }
  const common = await git(
    canonical,
    ["rev-parse", "--path-format=absolute", "--git-common-dir"],
    env,
    timeoutMs,
  );
  if (!common.ok || common.out === "") return null;
  const commonDir = isAbsolute(common.out) ? common.out : resolve(canonical, common.out);
  const id = createHash("sha256").update(canonical).digest("hex").slice(0, 24);
  return { path: join(commonDir, "toolu", "checkpoint", id), worktree: canonical };
}

async function prepareTree(
  storage: CheckpointStorage,
  head: string,
  env: NodeJS.ProcessEnv,
  timeoutMs: number,
): Promise<GitResult> {
  const metaPath = join(storage.path, "meta.json");
  const meta = readJson<CheckpointMeta | null>(metaPath, null);
  const savedIndex =
    meta?.version === 1
      ? "index"
      : meta?.version === 2 && /^index(?:-[0-9a-f-]{36})?$/.test(meta.index)
        ? meta.index
        : "index";
  let index = join(storage.path, savedIndex);
  let indexEnv = { ...env, ...IDENTITY, GIT_INDEX_FILE: index };
  const reusable =
    (meta?.version === 1 || meta?.version === 2) &&
    meta.worktree === storage.worktree &&
    meta.head === head &&
    existsSync(index) &&
    !existsSync(`${index}.lock`);
  if (!reusable) {
    if (existsSync(`${index}.lock`)) {
      index = join(storage.path, `index-${randomUUID()}`);
      indexEnv = { ...env, ...IDENTITY, GIT_INDEX_FILE: index };
    } else {
      rmSync(index, { force: true });
    }
    const initialized = await git(storage.worktree, ["read-tree", "HEAD"], indexEnv, timeoutMs);
    if (!initialized.ok) return initialized;
    writeJsonAtomic(metaPath, {
      version: 2,
      worktree: storage.worktree,
      head,
      index: index.slice(storage.path.length + 1),
    } satisfies CheckpointMeta);
  }
  let staged = await git(storage.worktree, ["add", "-A"], indexEnv, timeoutMs);
  if (!staged.ok && reusable) {
    if (existsSync(`${index}.lock`)) {
      index = join(storage.path, `index-${randomUUID()}`);
      indexEnv = { ...env, ...IDENTITY, GIT_INDEX_FILE: index };
    } else {
      rmSync(index, { force: true });
    }
    const reset = await git(storage.worktree, ["read-tree", "HEAD"], indexEnv, timeoutMs);
    if (reset.ok) {
      writeJsonAtomic(metaPath, {
        version: 2,
        worktree: storage.worktree,
        head,
        index: index.slice(storage.path.length + 1),
      } satisfies CheckpointMeta);
      staged = await git(storage.worktree, ["add", "-A"], indexEnv, timeoutMs);
    }
  }
  return staged.ok
    ? git(storage.worktree, ["write-tree"], indexEnv, timeoutMs)
    : { out: "", ok: false };
}

async function previousMatches(
  worktree: string,
  current: string,
  tree: string,
  head: string,
  env: NodeJS.ProcessEnv,
  timeoutMs: number,
): Promise<boolean> {
  const previousTree = await git(worktree, ["rev-parse", `${current}^{tree}`], env, timeoutMs);
  if (!previousTree.ok || previousTree.out !== tree) return false;
  if (current === head) return true;
  const previousParent = await git(worktree, ["rev-parse", `${current}^`], env, timeoutMs);
  return previousParent.ok && previousParent.out === head;
}

/** Create or refresh a crash-recovery ref without modifying the worker index. */
export async function snapshot(
  worktree: string,
  key: string,
  options: SnapshotOptions = {},
): Promise<Snapshot> {
  const ref = `refs/epic-wip/${key}`;
  const base: Snapshot = { key, ref, sha: null, dirty: false, unpushed: 0, changed: false };
  if (!worktree || !existsSync(join(worktree, ".git"))) return { ...base, skipped: "no worktree" };

  const commandTimeoutMs = positiveFinite(
    options.commandTimeoutMs,
    DEFAULT_COMMAND_TIMEOUT_MS,
    "commandTimeoutMs",
  );
  const lockTimeoutMs = positiveFinite(
    options.lockTimeoutMs,
    DEFAULT_LOCK_TIMEOUT_MS,
    "lockTimeoutMs",
  );
  const env = { ...process.env, ...options.env };
  const storage = await checkpointDirectory(worktree, env, commandTimeoutMs);
  if (!storage) return { ...base, skipped: "no git directory" };
  mkdirSync(storage.path, { recursive: true });
  const lock = await acquireLock(`${storage.path}.lock`, { timeoutMs: lockTimeoutMs });
  if (!lock) return { ...base, skipped: "checkpoint busy" };

  try {
    for (let attempt = 0; attempt < 2; attempt += 1) {
      const head = await git(storage.worktree, ["rev-parse", "HEAD"], env, commandTimeoutMs);
      if (!head.ok) return { ...base, skipped: "no HEAD" };
      const status = await git(storage.worktree, ["status", "--porcelain"], env, commandTimeoutMs);
      const ahead = await git(
        storage.worktree,
        ["rev-list", "--count", "HEAD", "--not", "--remotes=origin"],
        env,
        commandTimeoutMs,
      );
      if (!status.ok || !ahead.ok) return { ...base, skipped: "git failed" };
      const dirty = status.out !== "";
      const unpushed = Number(ahead.out);
      if (!Number.isSafeInteger(unpushed) || unpushed < 0) {
        return { ...base, dirty, skipped: "git failed" };
      }
      if (!dirty && unpushed === 0) return { ...base, skipped: "nothing at risk" };

      let sha = head.out;
      if (dirty) {
        const tree = await prepareTree(storage, head.out, env, commandTimeoutMs);
        if (!tree.ok) return { ...base, dirty, unpushed, skipped: "git failed" };

        const stableHead = await git(
          storage.worktree,
          ["rev-parse", "HEAD"],
          env,
          commandTimeoutMs,
        );
        if (!stableHead.ok) return { ...base, dirty, unpushed, skipped: "no HEAD" };
        if (stableHead.out !== head.out) {
          if (attempt === 0) continue;
          return { ...base, dirty, unpushed, skipped: "HEAD changed during checkpoint" };
        }

        const current = await git(
          storage.worktree,
          ["rev-parse", "--verify", "--quiet", ref],
          env,
          commandTimeoutMs,
        );
        if (
          current.ok &&
          (await previousMatches(
            storage.worktree,
            current.out,
            tree.out,
            head.out,
            env,
            commandTimeoutMs,
          ))
        ) {
          return { ...base, sha: current.out, dirty, unpushed };
        }
        const stamp = new Date().toISOString();
        const commit = await git(
          storage.worktree,
          ["commit-tree", tree.out, "-p", head.out, "-m", `epic-wip ${key} ${stamp}`],
          { ...env, ...IDENTITY },
          commandTimeoutMs,
        );
        if (!commit.ok) {
          return { ...base, dirty, unpushed, skipped: "commit-tree failed" };
        }
        const committedHead = await git(
          storage.worktree,
          ["rev-parse", "HEAD"],
          env,
          commandTimeoutMs,
        );
        if (!committedHead.ok) return { ...base, dirty, unpushed, skipped: "no HEAD" };
        if (committedHead.out !== head.out) {
          if (attempt === 0) continue;
          return { ...base, dirty, unpushed, skipped: "HEAD changed during checkpoint" };
        }
        sha = commit.out;
      }

      const current = await git(
        storage.worktree,
        ["rev-parse", "--verify", "--quiet", ref],
        env,
        commandTimeoutMs,
      );
      if (current.ok && current.out === sha) {
        return { ...base, sha, dirty, unpushed };
      }
      const updated = await git(
        storage.worktree,
        ["update-ref", "--create-reflog", "-m", "epic checkpoint", ref, sha],
        env,
        commandTimeoutMs,
      );
      if (!updated.ok) return { ...base, dirty, unpushed, skipped: "update-ref failed" };
      return { ...base, sha, dirty, unpushed, changed: true };
    }
    return { ...base, skipped: "HEAD changed during checkpoint" };
  } finally {
    lock.release();
  }
}

type Rec = { stage?: string; worktree?: string };

export async function snapshotActive(stateDir: string, only?: string): Promise<Snapshot[]> {
  let names: string[] = [];
  try {
    names = readdirSync(join(stateDir, "issues")).filter((name) => name.endsWith(".json"));
  } catch {
    return [];
  }
  const out: Snapshot[] = [];
  for (const name of names) {
    const key = name.slice(0, -5);
    if (only !== undefined && key !== only) continue;
    const rec = readJson<Rec>(join(stateDir, "issues", name), {});
    const active = rec.stage === "running" || rec.stage === "awaiting_merge";
    if (only === undefined && !active) continue;
    out.push(await snapshot(rec.worktree ?? "", key));
  }
  return out;
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  let stateDir: string | undefined;
  let key: string | undefined;
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--state-dir") stateDir = argv[++i];
    else if (arg === "--key") key = argv[++i];
    else throw new Error(`unknown arg: ${arg}`);
  }
  if (!stateDir) throw new Error("usage: checkpoint.ts --state-dir DIR [--key KEY]");
  process.stdout.write(`${JSON.stringify(await snapshotActive(stateDir, key), null, 2)}\n`);
}

if (import.meta.main) {
  main().catch((error: unknown) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exit(1);
  });
}
