#!/usr/bin/env bun
/** Reject new references to the four retired standalone plugins (#406). */
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "../..");
const SELF = "tooling/src/check-retired-plugin-references.ts";
const HISTORY = new Set([
  "CHANGELOG.md",
  "fixtures/gates/lifecycle.json", // exact historical parity inputs include retired-plugin names
  "fixtures/gates/lifecycle-golden.json",
  "fixtures/gates/pre-tool-modules-a.json", // preserves the former MCP/Jira case inputs
  "fixtures/gates/pre-tool-modules-a-golden.json",
  "plugins/pr-babysit/hooks/src/__tests__/fixtures/parse-verdict.golden.json",
  "plugins/pr-babysit/scripts/__tests__/fixtures/pr120-verdict-changes.txt",
  "plugins/toolu/hooks/src/__tests__/fixtures/pre-tool-modules-a-golden.json",
  "plugins/toolu/scripts/__tests__/fixtures/debug/big.log",
]);
const JIRA_INTEGRATION = new Set([
  ".claude-plugin/marketplace.json",
  "AGENTS.md",
  "fixtures/index.json", // preserves the existing Jira word case name for parity
  "README.md",
  "docs/conformance-report.md",
  "docs/epic-orchestrator/README.md",
  "docs/plugins/index.md",
  "knip.json",
  "packages/toolu-core/src/config/__tests__/settings.test.ts",
  "packages/toolu-core/src/gates/__tests__/mcp-blocker.test.ts",
  "packages/toolu-core/src/rest/rest.ts",
  "plugins/toolu/hooks/src/__tests__/lifecycle-golden.test.ts",
  "plugins/toolu/hooks/src/__tests__/pre-tool-modules-a-cases.ts",
  "plugins/toolu/hooks/src/__tests__/session-start-cases.ts",
  "plugins/toolu/hooks/src/__tests__/user-prompt-submit-cases.ts",
  "plugins/toolu/settings/README.md",
  "tooling/src/opencode-acceptance/__tests__/run.test.ts",
  "tooling/src/opencode-acceptance/run.ts",
  "tooling/src/pack-inventory.ts",
]);

function paths(term: string, literal = false): string[] {
  const result = spawnSync(
    "git",
    ["grep", "-l", "-i", literal ? "-F" : "-w", "-e", term, "--", "."],
    {
      cwd: ROOT,
      encoding: "utf8",
    },
  );
  if (result.status !== 0 && result.status !== 1) {
    throw new Error(`git grep ${term} failed: ${result.stderr}`);
  }
  return result.stdout.trim().split("\n").filter(Boolean);
}

const unexpected: string[] = [];
for (const term of ["exa-search", "context7", "agent-browser", "jira"]) {
  for (const path of paths(term, term !== "jira")) {
    if (
      path === SELF ||
      HISTORY.has(path) ||
      path.startsWith("docs/toolu/") ||
      path.startsWith("plugins/epic-orchestrator/")
    ) {
      continue;
    }
    if (term === "context7" && path === "tools/toolu-cli/src/catalog/__tests__/order.test.ts") {
      continue; // required unknown-plugin regression test
    }
    if (
      term === "jira" &&
      (JIRA_INTEGRATION.has(path) ||
        path.startsWith("crates/epic-orchestrator/") ||
        path.startsWith("tools/toolu-opencode/generated/"))
    ) {
      continue; // built-in tracker, its crate's verb and drift-checked generated copies
    }
    unexpected.push(`${path}: ${term}`);
  }
}
for (const phrase of [
  "plugins/jira/",
  "docs/jira/",
  "jira@toolu",
  "jira-jira",
  "install jira",
  "remove jira",
]) {
  for (const path of paths(phrase, true)) {
    if (path === SELF || HISTORY.has(path) || path.startsWith("docs/toolu/")) continue;
    unexpected.push(`${path}: standalone ${phrase}`);
  }
}
if (unexpected.length) {
  throw new Error(`retired plugin references:\n${unexpected.join("\n")}`);
}
process.stdout.write(
  "retired plugin references: only reviewed history and Jira integration remain\n",
);
