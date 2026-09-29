/**
 * push-waiver (#259): the native module against the shipped `push-waiver.sh`,
 * both under their dispatcher, from the same real repository. Every
 * `push-waiver.bats` scenario leaves the same waiver state and no stdout
 * (AC-1). Pushes the text lexer missed promote the waiver now (#283 item 8,
 * AC-3).
 */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { hookEnv } from "../../dispatch/__tests__/dispatch-harness.ts";
import { pushWaiverPend } from "../../ledger/push-waiver.ts";
import { diffSha } from "../../state/diff-sha.ts";
import { pushWaiverModule } from "../push-waiver.ts";
import { bashPayload, bothSides } from "./gates-harness.ts";

const WAIVER = ".claude/tmp/push-review/feat_example.waiver.json";
const PENDING = ".claude/tmp/push-review/feat_example.pending-waiver.json";

/** `feat/example` one commit ahead of `main`; returns the env the hook runs with. */
function repo(sb: Sandbox): Record<string, string> {
  sb.git("checkout", "-q", "-b", "feat/example");
  sb.write("feature.txt", "feature\n");
  sb.git("add", "-A");
  sb.git("commit", "-q", "-m", "feature");
  return {
    ...hookEnv(sb),
    STATE_DIR: sb.path(".claude/tmp/push-review"),
    PUSH_REVIEW_BASE: "main",
  };
}

function pend(sb: Sandbox, env: Record<string, string>): void {
  const sha = diffSha(sb.project, "main", { env }) ?? "";
  pushWaiverPend(sb.project, "feat_example", sha, "main", "no-state", { env });
}

const push = (command: string, response: Record<string, unknown> = { exit_code: 0 }) =>
  bashPayload(command, { stdout: "", interrupted: false, ...response });

type Scenario = {
  name: string;
  stdin: (sb: Sandbox) => string;
  pending?: boolean;
  /** Runs after the marker is written: new code the marker did not ask about. */
  after?: (sb: Sandbox) => void;
};

const PARITY: Scenario[] = [
  { name: "a successful push promotes the pending marker", stdin: () => push("git push") },
  {
    name: "a failed push leaves the pending marker",
    stdin: () => push("git push", { exit_code: 1 }),
  },
  {
    name: "an interrupted push promotes nothing",
    stdin: () => bashPayload("git push", { stdout: "", interrupted: true }),
  },
  {
    name: "a host that reports no exit code is treated as success",
    stdin: () => bashPayload("git push", { stdout: "", interrupted: false }),
  },
  {
    name: "a marker for older code is not cashed in",
    stdin: () => push("git push"),
    after: (sb) => {
      sb.write("more.txt", "more\n");
      sb.git("add", "-A");
      sb.git("commit", "-q", "-m", "more");
    },
  },
  { name: "no pending marker means no waiver", stdin: () => push("git push"), pending: false },
  { name: "a non-push command is ignored", stdin: () => push("git status") },
  {
    name: "a push inside a quoted heredoc body is ignored",
    stdin: () => push("cat <<'EOF' > notes.txt\ngit push\nEOF"),
  },
  { name: "git -C <path> push is recognised", stdin: (sb) => push(`git -C ${sb.project} push`) },
  {
    name: "a non-Bash tool exits silently",
    stdin: () =>
      JSON.stringify({ tool_name: "Edit", tool_input: { file_path: "a.ts" }, tool_response: {} }),
  },
];

async function run(s: Scenario) {
  using sb = createSandbox({ git: true });
  const env = repo(sb);
  if (s.pending !== false) pend(sb, env);
  s.after?.(sb);
  return bothSides(sb, "push-waiver", pushWaiverModule, [{ stdin: s.stdin(sb), env }]);
}

for (const s of PARITY) {
  test.concurrent(s.name, async () => {
    const { bash, ts } = await run(s);
    expect(ts).toEqual(bash);
    expect(ts.stdout).toBe("");
  });
}

test.concurrent("a promoted waiver names the pushed diff and drops the marker", async () => {
  const { ts } = await run(PARITY[0] ?? { name: "", stdin: () => "" });
  expect(ts.state[PENDING]).toBeUndefined();
  expect(JSON.parse(ts.state[WAIVER] ?? "{}")).toMatchObject({
    version: 1,
    branch: "feat_example",
    base_branch: "main",
    waived_at: "<T>",
  });
});

/** #283 item 8: pushes the bash lexer did not see. */
const MISSED_BY_BASH: Record<string, string> = {
  "283-8a push behind timeout": "timeout 120 git push",
  "283-8f git by path": "/usr/bin/git push",
  "283-8h push inside bash -c": 'bash -c "git push"',
  "283-8i push inside eval": "eval git push",
};

for (const [name, command] of Object.entries(MISSED_BY_BASH)) {
  test.concurrent(`#283: ${name} promotes the waiver`, async () => {
    const { bash, ts } = await run({ name, stdin: () => push(command) });
    expect(ts.state[WAIVER]).toBeDefined();
    // The bash baseline is the known-wrong one: it never saw the push.
    expect(bash.state[WAIVER]).toBeUndefined();
  });
}

test.concurrent("a dynamic git subcommand is not taken for a push", async () => {
  const { ts } = await run({ name: "", stdin: () => push("git $SUB") });
  expect(ts.state[WAIVER]).toBeUndefined();
});

/**
 * `push_target_root`: the push's `-C` chain picks the repository, and so the
 * branch and state the waiver is keyed to. A second worktree on `feat/wt`
 * sits beside the project; each repository holds a pending marker for its own
 * diff, in its own state directory (no `STATE_DIR`), and the default base
 * branch is detected (no `PUSH_REVIEW_BASE`).
 */
function twoRepos(sb: Sandbox): { env: Record<string, string>; wt: string } {
  const env = repo(sb);
  delete env.STATE_DIR;
  delete env.PUSH_REVIEW_BASE;
  const wt = join(sb.root, "wt");
  sb.git("worktree", "add", "-q", "-b", "feat/wt", wt, "main");
  const git = (...args: string[]) => spawnSync("git", ["-C", wt, ...args], { encoding: "utf8" });
  writeFileSync(join(wt, "wt.txt"), "wt\n");
  git("add", "-A");
  git("-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "wt");
  for (const [root, slug] of [
    [sb.project, "feat_example"],
    [wt, "feat_wt"],
  ] as const) {
    const sha = diffSha(root, "main", { env }) ?? "";
    pushWaiverPend(root, slug, sha, "main", "no-state", { env });
  }
  return { env, wt };
}

const WT_WAIVER = "../wt/.claude/tmp/push-review/feat_wt.waiver.json";
const PROJECT_WAIVER = ".claude/tmp/push-review/feat_example.waiver.json";

const CHAINS: Record<string, { command: string; promotes: string }> = {
  "a relative -C": { command: "git -C ../wt push", promotes: WT_WAIVER },
  "a cumulative -C chain": { command: "git -C .. -C wt push", promotes: WT_WAIVER },
  "an absolute -C": { command: "git -C <WT> push", promotes: WT_WAIVER },
  "no -C, with the base branch detected": { command: "git push", promotes: PROJECT_WAIVER },
};

for (const [name, { command, promotes }] of Object.entries(CHAINS)) {
  test.concurrent(`${name} is judged on the repository it pushes`, async () => {
    using sb = createSandbox({ git: true });
    const { env, wt } = twoRepos(sb);
    const stdin = push(command.replace("<WT>", wt));
    const { bash, ts } = await bothSides(sb, "push-waiver", pushWaiverModule, [{ stdin, env }], {
      also: [wt],
    });
    expect(ts).toEqual(bash);
    const waivers = Object.keys(ts.state).filter((path) => path.endsWith(".waiver.json"));
    expect(waivers).toEqual([promotes]);
  });
}

test.concurrent("a dynamic -C value falls back to the working directory's repository", async () => {
  using sb = createSandbox({ git: true });
  const { env, wt } = twoRepos(sb);
  const { bash, ts } = await bothSides(
    sb,
    "push-waiver",
    pushWaiverModule,
    [{ stdin: push('git -C "$WT" push'), env: { ...env, WT: wt } }],
    { also: [wt] },
  );
  expect(ts).toEqual(bash);
  // `$WT` is only known when the command runs, so the push is judged where the hook ran.
  const waivers = Object.keys(ts.state).filter((path) => path.endsWith(".waiver.json"));
  expect(waivers).toEqual([PROJECT_WAIVER]);
});
