/**
 * quality-gate cases (#261): every `@test` of the deleted quality-gate.bats.
 * The #283 item 8 fixtures and parity boundaries are in
 * `pre-tool-modules-b-quality-gate-283.ts`. The sandbox gate file says
 * "failing" unless a case says otherwise.
 */
import type { Fixture } from "@toolu/conformance/harness/fixtures";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { FAILING_GATE, gateMode, group, type ModuleCase } from "./pre-tool-modules-b-cases.ts";
import { QUALITY_GATE_283_CASES } from "./pre-tool-modules-b-quality-gate-283.ts";

const qg = group({ gate: FAILING_GATE });

const BLOCKED = "BLOCKED: quality gate failing";

const edit =
  (path: string): ((sb: Sandbox) => Fixture) =>
  (sb) => ({
    kind: "tool",
    event: "PreToolUse",
    toolName: "Edit",
    toolInput: {
      file_path: path.startsWith("/") ? path : sb.path(path),
      old_string: "a",
      new_string: "b",
    },
  });

const BATS_CASES: ModuleCase[] = [
  qg({
    name: "quality-gate: MY_CLAUDE_QUALITY=off switches it off",
    env: { MY_CLAUDE_QUALITY: "off" },
    command: 'git commit -m "wip"',
    expect: "advisory",
    lacks: [BLOCKED],
  }),
  qg({
    name: "quality-gate: no state file blocks nothing",
    gate: undefined,
    command: "echo hi",
    expect: "silent",
  }),
  qg({
    name: "quality-gate: reading a file is untouched",
    command: "cat tsconfig.json",
    expect: "silent",
  }),
  qg({
    name: "quality-gate: a destructive command is not this gate's business",
    setup: (sb) => void sb.write("bun.lock", ""),
    command: "rm -rf node_modules && bun run check",
    expect: "silent",
  }),
  qg({
    name: "quality-gate: git commit is denied during a failing gate",
    command: 'git commit -m "wip"',
    expect: "deny",
    has: [BLOCKED, "forced"],
  }),
  qg({
    name: "quality-gate: bun run check alone is allowed",
    setup: (sb) => void sb.write("bun.lock", ""),
    command: "bun run check",
    expect: "silent",
  }),
  qg({ name: "quality-gate: git status is allowed", command: "git status", expect: "silent" }),
  qg({
    name: "quality-gate: a destructive command in a bun+rust repo is out of scope",
    setup: (sb) => {
      sb.write("bun.lock", "");
      sb.write("Cargo.toml", "");
    },
    command: "rm -rf src && cargo fmt",
    expect: "silent",
  }),
  qg({
    name: "quality-gate: cargo fmt alone is allowed in a bun+rust repo",
    setup: (sb) => {
      sb.write("bun.lock", "");
      sb.write("Cargo.toml", "");
    },
    command: "cargo fmt",
    expect: "silent",
  }),
  qg({
    name: "quality-gate: bun run check && bun run build is allowed",
    setup: (sb) => void sb.write("bun.lock", ""),
    command: "bun run check && bun run build",
    expect: "silent",
  }),
  qg({
    name: "quality-gate: git push is denied during a failing gate",
    command: "git push origin feat/x",
    expect: "deny",
    has: [BLOCKED],
  }),
  qg({
    name: "quality-gate: git -C <path> commit is denied",
    fixture: (sb) => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "Bash",
      toolInput: { command: `git -C ${sb.project} commit -m "feat: x"` },
    }),
    expect: "deny",
    has: [BLOCKED],
  }),
  qg({
    name: "quality-gate: a commit reached through && is denied",
    command: 'git add -A && git commit -m "wip"',
    expect: "deny",
  }),
  qg({
    name: "quality-gate: a passing gate stops nothing",
    gate: '{"status":"passing"}\n',
    command: "echo ok",
    expect: "silent",
  }),
  qg({
    name: "quality-gate: an Edit tool call is untouched",
    fixture: edit("/tmp/notes.md"),
    expect: "silent",
  }),
  qg({
    name: "quality-gate: relaxed preset advises instead of denying",
    config: { version: 1, gates: { preset: "relaxed" } },
    command: 'git commit -m "wip"',
    expect: "advisory",
    has: ["quality gate is failing"],
    lacks: ["BLOCKED"],
  }),
  qg({
    name: "quality-gate: ask mode phrases the reason as a question",
    config: gateMode("qualityGate", "ask"),
    command: 'git commit -m "wip"',
    expect: "ask",
    has: ["anyway?"],
    lacks: ["BLOCKED"],
  }),
  qg({
    name: "quality-gate: a non-commit command never needs the config",
    config: "not json",
    command: "ls -la",
    expect: "silent",
  }),
  qg({
    name: "quality-gate: per-gate off emits nothing",
    config: gateMode("qualityGate", "off"),
    command: 'git commit -m "wip"',
    expect: "advisory",
    lacks: ["quality gate"],
  }),
  qg({
    name: "quality-gate: the failure reason and violations reach the user",
    gate: '{"status":"failing","reason":"oxlint: 2 errors","violations":"src/a.ts:1 no-explicit-any"}\n',
    command: 'git commit -m "wip"',
    expect: "deny",
    has: ["oxlint: 2 errors", "no-explicit-any"],
  }),
];

export const QUALITY_GATE_CASES: readonly ModuleCase[] = [...BATS_CASES, ...QUALITY_GATE_283_CASES];
