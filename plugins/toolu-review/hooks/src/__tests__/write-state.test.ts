/**
 * The write-state CLI bundle against real temp git repos (#269, ported from
 * state-writer.bats). The contract: its diff_sha, base, slug and
 * reviewed_files match the push-review gate's recipe, so the real bundle
 * accepts a clean state and denies an incomplete one.
 */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { entryArgv, launchedArgv } from "@toolu/conformance/harness/entry-command";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch, type RunResult } from "@toolu/conformance/harness/spawn";
import { z } from "zod";

const PLUGIN = resolve(import.meta.dir, "../../..");
const TOOLU_PLUGIN = resolve(PLUGIN, "../toolu");
const GATE = { plugin: "toolu", event: "PreToolUse", entry: "pre-tools" } as const;

const StateSchema = z.strictObject({
  version: z.literal(2),
  branch: z.string(),
  diff_sha: z.string(),
  base_branch: z.string(),
  reviewed_at: z.string().regex(/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/),
  reviewers: z.unknown(),
  findings_count: z.number(),
  findings: z.unknown(),
  review_round: z.number(),
  reviewed_files: z.array(z.string()),
});

function writeState(sb: Sandbox, args: string[], env: EnvPatch = {}, cwd = sb.project) {
  return run([...entryArgv("toolu-review", "write-state", PLUGIN), ...args], {
    cwd,
    env: { HOME: sb.home, ...env },
  });
}

/** The state file a successful run printed, schema-checked, with its raw bytes. */
function stateOf(res: RunResult) {
  expect(res.stderr).toBe("");
  expect(res.exitCode).toBe(0);
  const path = res.stdout.trimEnd();
  const text = readFileSync(path, "utf8");
  return { path, text, state: StateSchema.parse(JSON.parse(text)) };
}

function commit(sb: Sandbox, file: string, body: string): void {
  sb.write(file, body);
  sb.git("add", file);
  sb.git("commit", "-qm", `work ${file}`);
}

/** The gate's own recipe: `git diff --no-color BASE...HEAD | git hash-object --stdin`. */
function gateSha(repo: string, base: string): string {
  const diff = spawnSync("git", ["-C", repo, "diff", "--no-color", `${base}...HEAD`]);
  const hash = spawnSync("git", ["-C", repo, "hash-object", "--stdin"], { input: diff.stdout });
  return hash.stdout.toString().trim();
}

function feature(sb: Sandbox, branch = "feature"): void {
  sb.git("checkout", "-q", "-b", branch);
  commit(sb, "f.txt", "a\n");
}

/** The real PreToolUse bundle judging `command` from `sb.project`. */
async function gate(sb: Sandbox, command: string) {
  const payload = JSON.stringify({ tool_name: "Bash", tool_input: { command } });
  const res = await run(launchedArgv(GATE), {
    cwd: sb.project,
    env: {
      HOME: sb.home,
      CLAUDE_PLUGIN_ROOT: TOOLU_PLUGIN,
      TOOLU_BUN: process.execPath,
      PUSH_REVIEW_BASE: "main",
    },
    stdin: payload,
  });
  expect(res.exitCode).toBe(0);
  if (res.stdout.trim() === "") return { decision: "", reason: "" };
  const out = z
    .object({
      hookSpecificOutput: z.object({
        permissionDecision: z.string().optional(),
        permissionDecisionReason: z.string().optional(),
      }),
    })
    .parse(JSON.parse(res.stdout));
  return {
    decision: out.hookSpecificOutput.permissionDecision ?? "",
    reason: out.hookSpecificOutput.permissionDecisionReason ?? "",
  };
}

test.concurrent("diff_sha matches the gate recipe for a non-main base (origin/HEAD)", async () => {
  using sb = createSandbox({ git: true });
  sb.git("checkout", "-q", "-b", "develop");
  sb.git("commit", "--allow-empty", "-qm", "devbase");
  sb.git("update-ref", "refs/remotes/origin/develop", "refs/heads/develop");
  sb.git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/develop");
  feature(sb);
  const { state } = stateOf(
    await writeState(sb, ["--findings-count", "0"], { CLAUDE_PROJECT_DIR: sb.project }),
  );
  expect(state.base_branch).toBe("develop");
  expect(state.diff_sha).toBe(gateSha(sb.project, "develop"));
});

test.concurrent("diff_sha matches the gate recipe with the main fallback", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  const { state } = stateOf(await writeState(sb, ["--findings-count", "0"]));
  expect(state.base_branch).toBe("main");
  expect(state.diff_sha).toBe(gateSha(sb.project, "main"));
});

test.concurrent("$PUSH_REVIEW_BASE is honored, as the gate honors it", async () => {
  using sb = createSandbox({ git: true });
  sb.git("checkout", "-q", "-b", "develop");
  sb.git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/develop");
  feature(sb);
  const { state } = stateOf(
    await writeState(sb, ["--findings-count", "0"], { PUSH_REVIEW_BASE: "main" }),
  );
  expect(state.base_branch).toBe("main");
  expect(state.diff_sha).toBe(gateSha(sb.project, "main"));
});

test.concurrent("the slug maps feat/x-y to feat_x-y under .claude/tmp/push-review", async () => {
  using sb = createSandbox({ git: true });
  feature(sb, "feat/x-y");
  const res = await writeState(sb, ["--findings-count", "0"], { CLAUDE_PROJECT_DIR: sb.project });
  expect(stateOf(res).path).toBe(join(sb.project, ".claude/tmp/push-review/feat_x-y.json"));
});

test.concurrent("the Codex host stores the state under .codex/tmp", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  for (const env of [{ TOOLU_HOST_OVERRIDE: "codex" }, { PLUGIN_ROOT: PLUGIN }]) {
    const res = await writeState(sb, ["--findings-count", "0"], env);
    expect(stateOf(res).path).toBe(join(sb.project, ".codex/tmp/push-review/feature.json"));
  }
  expect(existsSync(join(sb.project, ".claude/tmp/push-review/feature.json"))).toBe(false);
});

test.concurrent("writes the v2 document as jq prints it and bumps review_round", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  const args = ["--findings-count", "0", "--reviewers", '["toolu-review:review"]'];
  const first = stateOf(await writeState(sb, args));
  expect(first.state).toMatchObject({
    version: 2,
    branch: "feature",
    findings_count: 0,
    reviewers: ["toolu-review:review"],
    findings: [],
    review_round: 1,
    reviewed_files: ["f.txt"],
  });
  expect(first.text).toBe(`${JSON.stringify(JSON.parse(first.text), null, 2)}\n`);
  expect(Object.keys(JSON.parse(first.text))).toEqual(Object.keys(StateSchema.shape));
  const second = stateOf(await writeState(sb, ["--findings-count", "0"]));
  expect(second.state.review_round).toBe(2);
});

test.concurrent("review_round restarts at 1 when the diff changes", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  await writeState(sb, ["--findings-count", "0"]);
  expect(stateOf(await writeState(sb, ["--findings-count", "0"])).state.review_round).toBe(2);
  commit(sb, "f.txt", "a\nb\n");
  const { state } = stateOf(await writeState(sb, ["--findings-count", "0"]));
  expect(state.review_round).toBe(1);
  expect(state.diff_sha).toBe(gateSha(sb.project, "main"));
});

test.concurrent("the state lands under the repo root, not $CLAUDE_PROJECT_DIR", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  const res = await writeState(sb, ["--findings-count", "0"], { CLAUDE_PROJECT_DIR: sb.home });
  expect(stateOf(res).path).toBe(join(sb.project, ".claude/tmp/push-review/feature.json"));
  expect(existsSync(join(sb.home, ".claude/tmp/push-review"))).toBe(false);
});

test.concurrent("--repo targets a worktree's own state dir", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  sb.git("checkout", "-q", "main");
  const wt = join(sb.project, "wt");
  sb.git("worktree", "add", "-q", wt, "feature");
  const { path, state } = stateOf(await writeState(sb, ["--findings-count", "0", "--repo", wt]));
  expect(path).toBe(join(wt, ".claude/tmp/push-review/feature.json"));
  expect(state.branch).toBe("feature");
  expect(state.diff_sha).toBe(gateSha(wt, "main"));
  expect(existsSync(join(sb.project, ".claude/tmp/push-review/feature.json"))).toBe(false);
});

test.concurrent("$STATE_DIR overrides the state directory, as in the gate", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  const custom = join(sb.root, "custom");
  const res = await writeState(sb, ["--findings-count", "0"], { STATE_DIR: custom });
  expect(stateOf(res).path).toBe(join(custom, "feature.json"));
});

test.concurrent("--repo outside a git repo fails loudly", async () => {
  using sb = createSandbox({ git: true });
  const outside = realpathSync(mkdtempSync(join(tmpdir(), "write-state-norepo-")));
  try {
    const res = await writeState(sb, ["--findings-count", "0", "--repo", outside]);
    expect(res.exitCode).toBe(1);
    expect(res.stderr).toBe(`write-state.sh: ${outside} is not inside a git repo\n`);
  } finally {
    rmSync(outside, { recursive: true, force: true });
  }
});

test.concurrent("an empty diff against base is refused and writes nothing", async () => {
  using sb = createSandbox({ git: true });
  sb.git("checkout", "-q", "-b", "feature");
  const res = await writeState(sb, ["--findings-count", "0"], { PUSH_REVIEW_BASE: "main" });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toBe("write-state.sh: diff against main is empty; nothing to review yet\n");
  expect(existsSync(join(sb.project, ".claude/tmp/push-review/feature.json"))).toBe(false);
});

test.concurrent("usage errors exit 2 with the bash wording", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  const cases: Array<[string[], string]> = [
    [["--findings-count", "notanumber"], "--findings-count must be an integer"],
    [[], "--findings-count required"],
    [["--findings-count", "0", "--bogus", "x"], "unknown arg: --bogus"],
    [["--findings-count"], "--findings-count needs a value"],
  ];
  for (const [args, message] of cases) {
    const res = await writeState(sb, args);
    expect(res.exitCode).toBe(2);
    expect(res.stderr).toBe(`write-state.sh: ${message}\n`);
  }
});

test.concurrent("malformed --reviewers or --findings JSON exits 1 and writes nothing", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  const res = await writeState(sb, ["--findings-count", "0", "--findings", "[oops"]);
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain("bad --reviewers/--findings/--reviewed-files JSON?");
  expect(existsSync(join(sb.project, ".claude/tmp/push-review/feature.json"))).toBe(false);
});

test.concurrent("reviewed_files is auto-computed from the real diff", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  commit(sb, "sub/g.txt", "b\n");
  commit(sb, "B.txt", "c\n");
  const res = await writeState(sb, ["--findings-count", "0"], { PUSH_REVIEW_BASE: "main" });
  expect(stateOf(res).state.reviewed_files).toEqual(["B.txt", "f.txt", "sub/g.txt"]);
});

test.concurrent("--reviewed-files replaces the auto list, deduped and sorted", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  commit(sb, "g.txt", "b\n");
  const only = await writeState(sb, ["--findings-count", "0", "--reviewed-files", "f.txt"]);
  expect(stateOf(only).state.reviewed_files).toEqual(["f.txt"]);
  const messy = await writeState(sb, [
    "--findings-count",
    "0",
    "--reviewed-files",
    "b.txt,a.txt,,a.txt",
  ]);
  expect(stateOf(messy).state.reviewed_files).toEqual(["a.txt", "b.txt"]);
});

test.concurrent("a clean v2 state passes the real push-review gate", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  commit(sb, "g.txt", "b\n");
  const args = ["--findings-count", "0", "--reviewers", '["toolu-review:review"]'];
  stateOf(await writeState(sb, args, { CLAUDE_PROJECT_DIR: sb.project, PUSH_REVIEW_BASE: "main" }));
  expect((await gate(sb, "git push origin feature")).decision).not.toBe("deny");
});

test.concurrent("a state missing a changed file is denied by the real gate", async () => {
  using sb = createSandbox({ git: true });
  sb.writeConfig("claude", "project", { version: 1, gates: { preset: "strict" } });
  feature(sb);
  commit(sb, "g.txt", "b\n");
  const args = ["--findings-count", "0", "--reviewed-files", "f.txt"];
  stateOf(await writeState(sb, args, { CLAUDE_PROJECT_DIR: sb.project, PUSH_REVIEW_BASE: "main" }));
  const verdict = await gate(sb, "git push origin feature");
  expect(verdict.decision).toBe("deny");
  expect(verdict.reason).toContain("g.txt");
});

test.concurrent("a detached worktree with --branch satisfies the gate for HEAD:<branch>", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  sb.git("checkout", "-q", "main");
  const wt = join(sb.project, "wt");
  sb.git("worktree", "add", "-q", "--detach", wt, "feature");
  const res = await writeState(sb, ["--repo", wt, "--branch", "feature", "--findings-count", "0"], {
    PUSH_REVIEW_BASE: "main",
  });
  const { path, state } = stateOf(res);
  expect(path).toBe(join(wt, ".claude/tmp/push-review/feature.json"));
  expect(state).toMatchObject({ branch: "feature", version: 2, diff_sha: gateSha(wt, "main") });
  expect((await gate(sb, `git -C ${wt} push origin HEAD:feature`)).decision).not.toBe("deny");
});

test.concurrent("a detached HEAD without --branch fails and says what to pass", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  sb.git("checkout", "-q", sb.git("rev-parse", "HEAD").trim());
  const res = await writeState(sb, ["--findings-count", "0"], { PUSH_REVIEW_BASE: "main" });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain("detached HEAD");
  expect(res.stderr).toContain("--branch");
});

test.concurrent("--branch must name a real ref and match an attached checkout", async () => {
  using sb = createSandbox({ git: true });
  feature(sb);
  sb.git("checkout", "-q", sb.git("rev-parse", "HEAD").trim());
  const env = { PUSH_REVIEW_BASE: "main" };
  const unknown = await writeState(sb, ["--branch", "nope", "--findings-count", "0"], env);
  expect(unknown.exitCode).toBe(1);
  expect(unknown.stderr).toContain("unknown branch 'nope'");
  const invalid = await writeState(sb, ["--branch", "a..b", "--findings-count", "0"], env);
  expect(invalid.exitCode).toBe(1);
  expect(invalid.stderr).toContain("--branch 'a..b' is not a valid branch name");
  sb.git("checkout", "-q", "feature");
  const other = await writeState(sb, ["--branch", "other", "--findings-count", "0"], env);
  expect(other.exitCode).toBe(1);
  expect(other.stderr).toContain("does not match the checked-out branch 'feature'");
  const same = await writeState(sb, ["--branch", "feature", "--findings-count", "0"], env);
  expect(stateOf(same).state.branch).toBe("feature");
});
