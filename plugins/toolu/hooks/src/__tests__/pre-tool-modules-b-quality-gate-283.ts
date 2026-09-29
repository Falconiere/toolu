/**
 * quality-gate cases (#261), continued: the #283 item 8 fixtures and parity
 * boundaries. The sandbox gate file says "failing" unless a case says otherwise.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { FAILING_GATE, gateMode, group, type ModuleCase } from "./pre-tool-modules-b-cases.ts";

const qg = group({ gate: FAILING_GATE });

const BLOCKED = "BLOCKED: quality gate failing";
const ITEM_8 = "#283 item 8: bash did not see this commit or push";

const ISSUE_283_CASES: ModuleCase[] = [
  qg({
    name: "quality-gate: a commit behind sudo (#283 item 8)",
    command: 'sudo -u me git commit -m "feat: x"',
    expect: "deny",
    has: [BLOCKED],
    deviation: ITEM_8,
  }),
  qg({
    name: "quality-gate: /usr/bin/git commit (#283 item 8)",
    command: '/usr/bin/git commit -m "feat: x"',
    expect: "deny",
    has: [BLOCKED],
    deviation: ITEM_8,
  }),
  qg({
    name: "quality-gate: a commit inside bash -c (#283 item 8)",
    command: "bash -c 'git commit -m \"feat: x\"'",
    expect: "deny",
    has: [BLOCKED],
    deviation: ITEM_8,
  }),
  qg({
    name: "quality-gate: a push behind timeout (#283 item 8)",
    command: "timeout 120 git push",
    expect: "deny",
    has: [BLOCKED],
    deviation: ITEM_8,
  }),
  qg({
    name: "quality-gate: a push through xargs (#283 item 8)",
    command: "echo main | xargs git push origin",
    expect: "deny",
    has: [BLOCKED],
    deviation: ITEM_8,
  }),
];

const BOUNDARY_CASES: ModuleCase[] = [
  qg({
    name: "quality-gate: non-string reason and violations print as jq -r does",
    gate: '{"status":"failing","reason":{"tool":"oxlint","errors":2},"violations":["a.ts","b.ts"]}\n',
    command: 'git commit -m "wip"',
    expect: "deny",
    has: ['"tool": "oxlint"', '[\n  "a.ts",'],
  }),
  qg({
    name: "quality-gate: outside a git repository the working directory is the root",
    gate: undefined,
    setup: (sb) => {
      const file = join(sb.root, "outside/.claude/tmp/quality-gate-status.json");
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, FAILING_GATE);
    },
    cwd: (sb) => join(sb.root, "outside"),
    command: 'git commit -m "wip"',
    expect: "deny",
    has: [BLOCKED],
  }),
  qg({
    name: "quality-gate: a codex apply_patch is untouched while the gate fails",
    host: "codex",
    fixture: (sb) => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "Edit",
      toolInput: { file_path: sb.path("src/fix.ts"), old_string: "a", new_string: "b" },
    }),
    expect: "silent",
  }),
  qg({
    name: "quality-gate: codex blocks a commit",
    host: "codex",
    command: 'git commit -m "wip"',
    expect: "deny",
    has: [BLOCKED],
  }),
  qg({
    name: "quality-gate: codex degrades ask to advice",
    host: "codex",
    config: gateMode("qualityGate", "ask"),
    command: 'git commit -m "wip"',
    expect: "advisory",
    has: ["Heads up: the quality gate is failing"],
    lacks: ["BLOCKED", "anyway?"],
  }),
  qg({
    name: "quality-gate: a malformed gate file is not failing",
    gate: "{not json",
    command: 'git commit -m "wip"',
    expect: "advisory",
    lacks: ["quality gate"],
  }),
  qg({
    name: "quality-gate: a gate file that is not an object is not failing",
    gate: '"failing"\n',
    command: 'git commit -m "wip"',
    expect: "advisory",
    lacks: ["quality gate"],
  }),
  qg({
    name: "quality-gate: a multi-slot gate file blocks with its joined violations",
    gate: `${JSON.stringify({ status: "failing", reason: "r2", source: "ts", file: "/p/b.ts", violations: "v1\nv2\n", entries: {}, updatedAt: "2026-09-29T00:00:00Z" })}\n`,
    command: 'git commit -m "wip"',
    expect: "deny",
    has: ["r2", "v1\nv2"],
  }),
  qg({
    name: "quality-gate: a null reason falls back to the default",
    gate: '{"status":"failing","reason":null}\n',
    command: 'git commit -m "wip"',
    expect: "deny",
    has: ["Quality gate failing"],
  }),
  qg({
    name: "quality-gate: the Shell tool's commit is denied",
    fixture: () => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "Shell",
      toolInput: { command: 'git commit -m "wip"' },
    }),
    expect: "deny",
    has: [BLOCKED],
  }),
  qg({
    name: "quality-gate: a commit-shaped heredoc body is not a commit",
    command: 'cat <<EOF\ngit commit -m "wip"\nEOF',
    expect: "silent",
  }),
  qg({
    name: "quality-gate: a linked worktree is skipped",
    setup: (sb) => {
      sb.git("worktree", "add", "-q", "-b", "feat/wt", join(sb.root, "wt"));
      const file = join(sb.root, "wt/.claude/tmp/quality-gate-status.json");
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, FAILING_GATE);
    },
    cwd: (sb) => join(sb.root, "wt"),
    command: 'git commit -m "wip"',
    expect: "advisory",
    lacks: ["quality gate"],
  }),
];

export const QUALITY_GATE_283_CASES: readonly ModuleCase[] = [
  ...ISSUE_283_CASES,
  ...BOUNDARY_CASES,
];
