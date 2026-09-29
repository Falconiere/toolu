/**
 * bash-commands cases (#261), continued: the #283 items 3 and 4 fixtures and
 * parity boundaries. `shipped` runs against the shipped deny list, in block mode.
 */
import {
  bashLists as lists,
  group,
  gateMode,
  type ModuleCase,
} from "./pre-tool-modules-b-cases.ts";

const BLOCK = gateMode("bashCommands", "block");
const bc = group({ config: BLOCK, settings: lists("", "") });
const shipped = group({ config: BLOCK, shippedSettings: true });

const ITEM_3 = "#283 item 3: bash never looked inside bash -c or eval";
const ITEM_4 = "#283 item 4: bash matched a rule against the line's first word only";

const ISSUE_283_CASES: ModuleCase[] = [
  shipped({
    name: "bash-commands: bash -lc (#283 item 3)",
    command: "bash -lc 'node -e 1'",
    expect: "deny",
    has: ["node -e"],
    deviation: ITEM_3,
  }),
  shipped({
    name: "bash-commands: eval (#283 item 3)",
    command: 'eval "node -e 1"',
    expect: "deny",
    has: ["node -e"],
    deviation: ITEM_3,
  }),
  shipped({
    name: "bash-commands: after && (#283 item 4)",
    command: "cd /tmp && node -e 1",
    expect: "deny",
    has: ["node -e"],
    deviation: ITEM_4,
  }),
  shipped({
    name: "bash-commands: behind an env prefix (#283 item 4)",
    command: "FOO=1 node -e 1",
    expect: "deny",
    has: ["node -e"],
    deviation: ITEM_4,
  }),
  shipped({
    name: "bash-commands: in a subshell (#283 item 4)",
    command: "(node -e 1)",
    expect: "deny",
    has: ["node -e"],
    deviation: ITEM_4,
  }),
  shipped({
    name: "bash-commands: behind sudo (#283 item 4)",
    command: "sudo node -e 1",
    expect: "deny",
    has: ["node -e"],
    deviation: ITEM_4,
  }),
  shipped({
    name: "bash-commands: cargo test after cd (#283 item 4)",
    command: "cd crate && cargo test",
    expect: "deny",
    has: ["cargo test"],
    deviation: ITEM_4,
  }),
  shipped({
    name: "bash-commands: after a pipe (#283 item 4)",
    command: "echo x | node -e 1",
    expect: "deny",
    has: ["node -e"],
    deviation: ITEM_4,
  }),
  shipped({
    name: "bash-commands: after ; (#283 item 4)",
    command: "true; node -e 1",
    expect: "deny",
    has: ["node -e"],
    deviation: ITEM_4,
  }),
  shipped({
    name: "bash-commands: a rule does not match across two commands (#283 item 4)",
    command: 'bun install && node -e "1"',
    expect: "deny",
    has: ["deny rule: node -e"],
    lacks: ["bun -e"],
    deviation: `${ITEM_4}; it reported bun -e from bun + a later -e`,
  }),
  bc({
    name: "bash-commands: allow on another command does not override (#283 item 4)",
    settings: lists("ls", "node -e"),
    command: "ls && node -e 1",
    expect: "deny",
    has: ["node -e"],
    deviation: `${ITEM_4}; any allow match anywhere on the line overrode the deny`,
  }),
  shipped({
    name: "bash-commands: node -e without python3 on PATH (#283 item 4)",
    config: { version: 1 },
    noPython: true,
    command: "node -e 1",
    expect: "ask",
    has: ["node -e"],
    deviation: `${ITEM_4}; without python3 every multi-token rule was silently off`,
  }),
];

const BOUNDARY_CASES: ModuleCase[] = [
  shipped({
    name: "bash-commands: a heredoc body is not a command",
    command: "cat <<EOF\nnode -e 1\nEOF",
    expect: "silent",
  }),
  shipped({
    name: "bash-commands: a commit message naming node -e is not denied",
    command: 'git commit -m "fix: drop node -e usage"',
    expect: "advisory",
    lacks: ["deny rule"],
  }),
  shipped({
    name: "bash-commands: the Shell tool is checked too",
    fixture: () => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "Shell",
      toolInput: { command: "node -e 1" },
    }),
    expect: "deny",
  }),
  shipped({
    name: "bash-commands: an Edit tool call is out of scope",
    fixture: (sb) => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "Edit",
      toolInput: { file_path: sb.path("notes.md"), old_string: "a", new_string: "b" },
    }),
    expect: "silent",
  }),
  bc({
    name: "bash-commands: a single-token rule matches a redirect target",
    settings: lists("", "biome"),
    command: "echo x > biome.txt",
    expect: "deny",
    has: ["biome"],
  }),
];

export const BASH_COMMANDS_283_CASES: readonly ModuleCase[] = [
  ...ISSUE_283_CASES,
  ...BOUNDARY_CASES,
];
