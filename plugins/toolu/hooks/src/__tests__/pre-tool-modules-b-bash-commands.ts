/**
 * bash-commands cases (#261): every `@test` of the deleted bash-commands.bats.
 * The #283 items 3 and 4 fixtures and parity boundaries are in
 * `pre-tool-modules-b-bash-commands-283.ts`. The bats suite pinned `block` so
 * matches read as denies; the delivery-mode cases override it.
 */
import { BASH_COMMANDS_283_CASES } from "./pre-tool-modules-b-bash-commands-283.ts";
import {
  bashLists as lists,
  group,
  gateMode,
  type ModuleCase,
} from "./pre-tool-modules-b-cases.ts";

const bc = group({ config: gateMode("bashCommands", "block"), settings: lists("", "") });

const BATS_CASES: ModuleCase[] = [
  bc({ name: "bash-commands: accepts an allowed command", command: "ls -la", expect: "silent" }),
  bc({
    name: "bash-commands: rejects a denied command (substring match)",
    settings: lists("", "biome"),
    command: "biome check .",
    expect: "deny",
    has: ["Command blocked by deny rule: biome"],
  }),
  bc({
    name: "bash-commands: rejects node -e argv-aware",
    settings: lists("", "node -e"),
    command: 'node -e "console.log(1)"',
    expect: "deny",
    has: ["node -e"],
  }),
  bc({
    name: "bash-commands: rejects git push --force origin feat/x",
    settings: lists("", "git push --force"),
    command: "git push --force origin feat/x",
    expect: "deny",
    has: ["git push --force"],
  }),
  bc({
    name: "bash-commands: no-op when data files are missing",
    settings: {},
    command: "node -e 'rm -rf /'",
    expect: "silent",
  }),
  bc({
    name: "bash-commands: bare node script.js does not trip node -e",
    settings: lists("", "node -e"),
    command: "node script.js",
    expect: "silent",
  }),
  bc({
    name: "bash-commands: deny+allow, the allowlist entry overrides the deny",
    settings: lists("biome", "biome"),
    command: "biome check .",
    expect: "silent",
  }),
  bc({
    name: "bash-commands: deny+allow, argv-aware allow override of node -e",
    settings: lists("node -e", "node -e"),
    command: 'node -e "console.log(1)"',
    expect: "silent",
  }),
  bc({
    name: "bash-commands: an allow rule that does not match does not override",
    settings: lists("biome", "node -e"),
    command: 'node -e "console.log(1)"',
    expect: "deny",
  }),
  bc({
    name: "bash-commands: allow-only, an allowlisted command passes",
    settings: lists("ls", ""),
    command: "ls -la",
    expect: "silent",
  }),
  bc({
    name: "bash-commands: neither list matches, default allow",
    settings: lists("biome", "node -e"),
    command: "git status",
    expect: "silent",
  }),
  bc({
    name: "bash-commands: a broad node allow entry overrides the node -e deny",
    settings: lists("node", "node -e"),
    command: 'node -e "console.log(1)"',
    expect: "silent",
  }),
  bc({
    name: "bash-commands: empty command is allowed",
    settings: lists("", "cargo test"),
    command: "",
    expect: "silent",
  }),
  bc({
    name: "bash-commands: whitespace-only command is allowed",
    settings: lists("", "node -e"),
    command: "   ",
    expect: "silent",
  }),
  bc({
    name: "bash-commands: node script.js with node allowed and node -e denied",
    settings: lists("node", "node -e"),
    command: "node script.js",
    expect: "silent",
  }),
  bc({
    name: "bash-commands: a commit message mentioning cargo test is not denied",
    settings: lists("", "cargo test"),
    command: 'git commit -m "fix cargo test failure"',
    expect: "advisory",
    lacks: ["deny rule"],
  }),
  bc({
    name: "bash-commands: cargo --verbose test is denied via the argv rule",
    settings: lists("", "cargo test"),
    command: "cargo --verbose test",
    expect: "deny",
  }),
  bc({
    name: "bash-commands: mycargo test is allowed",
    settings: lists("", "cargo test"),
    command: "mycargo test",
    expect: "silent",
  }),
  bc({
    name: "bash-commands: the shipped default asks",
    settings: lists("", "node -e"),
    config: { version: 1 },
    command: 'node -e "console.log(1)"',
    expect: "ask",
    has: ["SECURITY GUARDRAIL"],
  }),
  bc({
    name: "bash-commands: advise mode only warns",
    settings: lists("", "node -e"),
    config: gateMode("bashCommands", "advise"),
    command: 'node -e "console.log(1)"',
    expect: "advisory",
    has: ["The command was not stopped"],
  }),
  bc({
    name: "bash-commands: ask mode prompts with the guardrail banner",
    settings: lists("", "node -e"),
    config: gateMode("bashCommands", "ask"),
    command: 'node -e "console.log(1)"',
    expect: "ask",
    has: ["SECURITY GUARDRAIL — OVERRIDE REQUESTED", "node -e", "full shell privileges"],
  }),
  bc({
    name: "bash-commands: strict preset restores the hard deny",
    settings: lists("", "node -e"),
    config: { version: 1, gates: { preset: "strict" } },
    command: 'node -e "console.log(1)"',
    expect: "deny",
  }),
  bc({
    name: "bash-commands: off turns the denylist into a no-op",
    settings: lists("", "node -e"),
    config: gateMode("bashCommands", "off"),
    command: 'node -e "console.log(1)"',
    expect: "silent",
  }),
  bc({
    name: "bash-commands: an allowlisted command is silent whatever the mode",
    settings: lists("node -e", "node -e"),
    config: { version: 1 },
    command: 'node -e "console.log(1)"',
    expect: "silent",
  }),
  bc({
    name: "bash-commands: ask degrades to block on codex",
    host: "codex",
    settings: lists("", "node -e"),
    config: gateMode("bashCommands", "ask"),
    command: 'node -e "x"',
    expect: "deny",
  }),
];

export const BASH_COMMANDS_CASES: readonly ModuleCase[] = [
  ...BATS_CASES,
  ...BASH_COMMANDS_283_CASES,
];
