/**
 * PreToolUse corpus (#258), shell, search, MCP and subagent tools: the
 * command gates, the ast-grep registry module, a registry exit 2, the
 * standalone mcp-blocker and agent-tier hooks, and malformed input.
 */
import { chmodSync } from "node:fs";
import { join } from "node:path";
import { abs, featureBranch, mode, stateDir, writeFile, type PretoolCase } from "./pretool-case.ts";
import { bashFixture, editFixture, mcpFixture } from "./fixtures.ts";
import { hostConfigRoot } from "./pretool.ts";

export const SHELL_CASES: PretoolCase[] = [
  {
    name: "bash-commands: denylisted command asks",
    entry: "pre-tools",
    fixture: () => bashFixture('bun -e "1"'),
    expect: { claude: "ask", codex: "deny" },
  },
  {
    name: "bash-commands: block mode denies",
    entry: "pre-tools",
    fixture: () => bashFixture("cargo test"),
    config: mode("bashCommands", "block"),
    expect: { claude: "deny" },
  },
  {
    name: "bash-commands: advise mode advises",
    entry: "pre-tools",
    fixture: () => bashFixture("cargo test"),
    config: mode("bashCommands", "advise"),
    expect: { claude: "advisory" },
  },
  {
    name: "plain command is silent",
    entry: "pre-tools",
    fixture: () => bashFixture("ls"),
    expect: { claude: "silent" },
  },
  {
    name: "commit-gate: commit advice",
    entry: "pre-tools",
    fixture: () => bashFixture('git commit -m "feat: x"'),
    expect: { claude: "advisory" },
  },
  {
    name: "commit-gate: unknown prefix denied in block mode",
    entry: "pre-tools",
    fixture: () => bashFixture('git commit -m "wibble: stuff"'),
    config: mode("commitGate", "block"),
    expect: { claude: "deny" },
  },
  {
    name: "quality-gate: failing gate blocks a commit",
    entry: "pre-tools",
    fixture: () => bashFixture('git commit -m "wip"'),
    setup: (sb, host) =>
      writeFile(
        sb.path(`${stateDir(host)}/tmp/quality-gate-status.json`),
        '{"status":"failing","reason":"forced","violations":""}\n',
      ),
    expect: { claude: "deny" },
  },
  {
    name: "quality-gate: ask mode",
    entry: "pre-tools",
    fixture: () => bashFixture("git push"),
    config: mode("qualityGate", "ask"),
    setup: (sb, host) =>
      writeFile(
        sb.path(`${stateDir(host)}/tmp/quality-gate-status.json`),
        '{"status":"failing","reason":"forced","violations":""}\n',
      ),
    expect: { claude: "ask", codex: "advisory" },
  },
  {
    name: "push-review + docs-sync: unreviewed push advises",
    entry: "pre-tools",
    fixture: () => bashFixture("git push origin feat/example"),
    setup: (sb) => featureBranch(sb),
    expect: { claude: "advisory" },
  },
  {
    name: "push-review: block mode denies",
    entry: "pre-tools",
    fixture: () => bashFixture("git push origin feat/example"),
    config: mode("pushReview", "block"),
    setup: (sb) => featureBranch(sb),
    expect: { claude: "deny" },
  },
  {
    name: "push-review: ask mode",
    entry: "pre-tools",
    fixture: () => bashFixture("git push origin feat/example"),
    config: mode("pushReview", "ask"),
    setup: (sb) => featureBranch(sb),
    expect: { claude: "ask", codex: "advisory" },
  },
  {
    name: "docs-sync: block mode denies a code-only push",
    entry: "pre-tools",
    fixture: () => bashFixture("git push origin feat/example"),
    config: { version: 1, gates: { docsSync: { mode: "block" }, pushReview: { mode: "off" } } },
    setup: (sb) => featureBranch(sb),
    expect: { claude: "deny" },
  },
  {
    name: "plan-ledger: unparseable ledger denies",
    entry: "pre-tools",
    fixture: () => bashFixture("git push origin feat/example"),
    setup: (sb, host) => {
      featureBranch(sb, "README.md");
      writeFile(sb.path(`${stateDir(host)}/tmp/plan-ledger/feat_example.json`), "{not json");
    },
    expect: { claude: "deny" },
  },
  {
    name: "ast-grep registry: structural Grep nudge",
    entry: "pre-tools",
    fixture: () => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "Grep",
      toolInput: { pattern: "fn handle_request", glob: "*.rs" },
    }),
    expect: { claude: "advisory" },
  },
  {
    name: "ast-grep registry: grep in Bash nudge",
    entry: "pre-tools",
    fixture: () => bashFixture('grep -r "impl Foo" src'),
    expect: { claude: "advisory" },
  },
  {
    name: "registry module exit 2 blocks",
    entry: "pre-tools",
    fixture: () => bashFixture("ls"),
    setup: (sb, host) => {
      const file = join(hostConfigRoot(sb, host), "toolu/pre-tools.d/fixture@toolu__hard-block.sh");
      writeFile(file, "#!/usr/bin/env bash\necho hard-block >&2\nexit 2\n");
      chmodSync(file, 0o755);
    },
    expect: { claude: "exit2" },
  },
  {
    name: "hooks.pre-tools false disables the dispatcher",
    entry: "pre-tools",
    fixture: (sb) => editFixture(abs(".env")(sb), "a", "b"),
    config: { version: 1, hooks: { "pre-tools": false } },
    expect: { claude: "silent" },
  },
  {
    name: "empty stdin",
    entry: "pre-tools",
    fixture: () => bashFixture("ls"),
    stdin: "",
    expect: { claude: "silent" },
  },
  {
    name: "non-JSON stdin",
    entry: "pre-tools",
    fixture: () => bashFixture("ls"),
    stdin: "not json\n",
    expect: { claude: "silent" },
  },
  {
    name: "mcp-blocker: config-disabled server asks",
    entry: "pre-tools-mcp",
    fixture: () => mcpFixture("exampleblocked", "search", {}),
    config: { version: 1, mcp: { exampleblocked: false } },
    expect: { claude: "ask", codex: "deny" },
  },
  {
    name: "mcp-blocker: blocklisted server with redirect, block mode",
    entry: "pre-tools-mcp",
    fixture: () => mcpFixture("exampleblocked", "search", {}),
    settings: { "mcp-blocklist.txt": "exampleblocked -> use the CLI\n" },
    config: mode("mcpBlocker", "block"),
    expect: { claude: "deny" },
  },
  {
    name: "mcp-blocker: unlisted server is silent",
    entry: "pre-tools-mcp",
    fixture: () => mcpFixture("github", "search", {}),
    expect: { claude: "silent" },
  },
  {
    name: "mcp-blocker: also runs inside the dispatcher",
    entry: "pre-tools",
    fixture: () => mcpFixture("exampleblocked", "search", {}),
    config: { version: 1, mcp: { exampleblocked: false } },
    expect: { claude: "ask", codex: "deny" },
  },
  {
    name: "agent-tier: tier mismatch advises",
    entry: "pre-tools-agent",
    fixture: () => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "Agent",
      toolInput: { model: "opus", subagent_type: "x", prompt: "p" },
    }),
    setup: (sb, host) => {
      featureBranch(sb);
      writeFile(
        sb.path(`${stateDir(host)}/tmp/plan-ledger/feat_example.json`),
        '{"version":1,"next":"s1","steps":[{"id":"s1","status":"running","model":"haiku"}]}\n',
      );
    },
    expect: { claude: "advisory" },
  },
  {
    name: "agent-tier: no ledger is silent",
    entry: "pre-tools-agent",
    fixture: () => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "Task",
      toolInput: { model: "opus", prompt: "p" },
    }),
    expect: { claude: "silent" },
  },
];
