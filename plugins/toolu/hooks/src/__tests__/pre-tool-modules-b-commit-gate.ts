/**
 * commit-gate cases (#261): every `@test` of the deleted commit-gate.bats, then
 * the #283 items 5 and 8 fixtures and parity boundaries. The bats suite pinned
 * `block`, so an unknown prefix reads as a deny.
 */
import { group, gateMode, type ModuleCase } from "./pre-tool-modules-b-cases.ts";

const PREFIXES = { "commit-prefixes.txt": "feat\nfix\nchore\ndocs\nrefactor\ntest\n" };

const cg = group({ config: gateMode("commitGate", "block"), settings: PREFIXES });

const UNKNOWN = "Unknown Conventional Commits prefix";
const ADVICE = "BEFORE COMMITTING";
const ITEM_5 = "#283 item 5: bash's sed extraction found no message in this form";
const ITEM_8 = "#283 item 8: bash did not see this commit";

const BATS_CASES: ModuleCase[] = [
  cg({
    name: "commit-gate: accepts a feat: prefix",
    command: 'git commit -m "feat: add widget"',
    expect: "advisory",
    has: [ADVICE, "git diff --stat against main"],
  }),
  cg({
    name: "commit-gate: rejects an unknown prefix",
    command: 'git commit -m "wibble: stuff"',
    expect: "deny",
    has: [`${UNKNOWN}: "wibble"`, "Base branch: main"],
  }),
  cg({
    name: "commit-gate: accepts fix: with escaped quotes",
    command: 'git commit -m "fix: handle \\"quoted\\" text"',
    expect: "advisory",
    has: [ADVICE],
  }),
  cg({
    name: "commit-gate: rejects an unknown prefix with escaped quotes",
    command: 'git commit -m "wibble: a \\"b\\" c"',
    expect: "deny",
  }),
  cg({
    name: "commit-gate: rejects an unknown prefix whose scope has escaped quotes",
    command: 'git commit -m "wibble(\\"ui\\"): add stuff"',
    expect: "deny",
  }),
  cg({
    name: "commit-gate: accepts a known prefix whose scope has escaped quotes",
    command: 'git commit -m "feat(\\"ui\\"): add stuff"',
    expect: "advisory",
  }),
  cg({
    name: "commit-gate: accepts a single-quoted -m message",
    command: "git commit -m 'feat: single quoted'",
    expect: "advisory",
  }),
  cg({
    name: "commit-gate: the shipped default advises on a bad prefix",
    config: { version: 1 },
    command: 'git commit -m "wip: something"',
    expect: "advisory",
    has: [UNKNOWN],
    lacks: [ADVICE],
  }),
  cg({
    name: "commit-gate: off emits nothing",
    config: gateMode("commitGate", "off"),
    command: 'git commit -m "wip: something"',
    expect: "silent",
  }),
  cg({
    name: "commit-gate: git -C <path> commit is gated",
    command: 'git -C /tmp/wt commit -m "wip: something"',
    expect: "deny",
  }),
  cg({
    name: "commit-gate: a commit reached through && is gated",
    command: 'git add -A && git commit -m "wip: something"',
    expect: "deny",
  }),
  cg({
    name: "commit-gate: a commit-shaped heredoc body is not a commit",
    command: 'cat <<EOF > notes.txt\ngit commit -m "wip: nope"\nEOF',
    expect: "silent",
  }),
];

const ISSUE_283_CASES: ModuleCase[] = [
  cg({
    name: "commit-gate: the $(cat <<'EOF') message form (#283 item 5)",
    command: "git commit -m \"$(cat <<'EOF'\nwibble: x\n\nbody\nEOF\n)\"",
    expect: "deny",
    has: [`${UNKNOWN}: "wibble"`],
    deviation: ITEM_5,
  }),
  cg({
    name: "commit-gate: -am (#283 item 5)",
    command: 'git commit -am "wibble: x"',
    expect: "deny",
    has: [UNKNOWN],
    deviation: ITEM_5,
  }),
  cg({
    name: "commit-gate: -m'…' (#283 item 5)",
    command: "git commit -m'wibble: x'",
    expect: "deny",
    has: [UNKNOWN],
    deviation: ITEM_5,
  }),
  cg({
    name: "commit-gate: --message= (#283 item 5)",
    command: 'git commit --message="wibble: x"',
    expect: "deny",
    has: [UNKNOWN],
    deviation: ITEM_5,
  }),
  cg({
    name: "commit-gate: the subject is the first -m (#283 item 5)",
    command: 'git commit -m "wibble: x" -m "feat: body"',
    expect: "deny",
    has: [`${UNKNOWN}: "wibble"`],
    deviation: "#283 item 5: bash's greedy sed checked the last -m, which git records as the body",
  }),
  cg({
    name: "commit-gate: behind sudo (#283 item 8)",
    command: 'sudo -u me git commit -m "wibble: x"',
    expect: "deny",
    has: [UNKNOWN],
    deviation: ITEM_8,
  }),
  cg({
    name: "commit-gate: /usr/bin/git (#283 item 8)",
    command: '/usr/bin/git commit -m "wibble: x"',
    expect: "deny",
    has: [UNKNOWN],
    deviation: ITEM_8,
  }),
  cg({
    name: "commit-gate: inside bash -c (#283 item 8)",
    command: "bash -c 'git commit -m \"wibble: x\"'",
    expect: "deny",
    has: [UNKNOWN],
    deviation: ITEM_8,
  }),
];

const BOUNDARY_CASES: ModuleCase[] = [
  cg({
    name: "commit-gate: no prefix file skips the prefix check",
    settings: {},
    command: 'git commit -m "wibble: x"',
    expect: "advisory",
    has: [ADVICE],
    lacks: [UNKNOWN],
  }),
  cg({
    name: "commit-gate: a message without a type is not checked",
    command: 'git commit -m "wip"',
    expect: "advisory",
    has: [ADVICE],
  }),
  cg({
    name: "commit-gate: a breaking-change bang is not parsed as a type",
    command: 'git commit -m "wibble!: x"',
    expect: "advisory",
    has: [ADVICE],
  }),
  cg({
    name: "commit-gate: a dynamic message is not checked",
    command: 'git commit -m "$MSG"',
    expect: "advisory",
    has: [ADVICE],
  }),
  cg({
    name: "commit-gate: an editor-driven commit only gets the reminder",
    command: "git commit",
    expect: "advisory",
    has: [ADVICE],
  }),
  cg({
    name: "commit-gate: ask mode prompts on claude",
    config: gateMode("commitGate", "ask"),
    command: 'git commit -m "wibble: x"',
    expect: "ask",
    has: [UNKNOWN],
  }),
  cg({
    name: "commit-gate: ask degrades to advice on codex",
    host: "codex",
    config: gateMode("commitGate", "ask"),
    command: 'git commit -m "wibble: x"',
    expect: "advisory",
    has: [UNKNOWN],
  }),
  cg({
    name: "commit-gate: a relaxed preset turns the gate off",
    config: { version: 1, gates: { preset: "relaxed" } },
    command: 'git commit -m "wibble: x"',
    expect: "silent",
  }),
  cg({
    name: "commit-gate: the Shell tool is not a commit-gate tool",
    fixture: () => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "Shell",
      toolInput: { command: 'git commit -m "wibble: x"' },
    }),
    expect: "silent",
  }),
  cg({
    name: "commit-gate: the base branch comes from origin/HEAD",
    setup: (sb) => {
      sb.git("update-ref", "refs/remotes/origin/develop", "HEAD");
      sb.git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/develop");
    },
    command: 'git commit -m "feat: x"',
    expect: "advisory",
    has: ["against develop"],
  }),
  cg({
    name: "commit-gate: an invalid config envelope blocks",
    config: { version: 2 },
    command: 'git commit -m "wibble: x"',
    expect: "deny",
    has: [UNKNOWN],
    deviation:
      "#253: loadConfig rejects an unsupported envelope and every gate blocks; bash read it as {}",
  }),
];

export const COMMIT_GATE_CASES: readonly ModuleCase[] = [
  ...BATS_CASES,
  ...ISSUE_283_CASES,
  ...BOUNDARY_CASES,
];
