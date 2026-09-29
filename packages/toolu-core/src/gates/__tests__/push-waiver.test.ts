/**
 * push-waiver (#259): the native module against the shipped `push-waiver.sh`,
 * both under their dispatcher, from the same real repository. Every
 * `push-waiver.bats` scenario leaves the same waiver state and no stdout
 * (AC-1). Pushes the text lexer missed promote the waiver now (#283 item 8,
 * AC-3).
 */
import { expect, test } from "bun:test";
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
