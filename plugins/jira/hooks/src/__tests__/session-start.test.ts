/** jira's SessionStart bundle through its real hooks.json launcher (#269, ported from session-start.bats). */
import { resolve } from "node:path";
import { publishedCliSuite } from "@toolu/conformance/harness/startup";

publishedCliSuite({
  plugin: "jira",
  pluginRoot: resolve(import.meta.dir, "../../.."),
  source: "hooks/dist/jira.js",
  dir: "jira",
  name: "jira.sh",
  advisory:
    "jira: bun not found on PATH — the jira CLI needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)",
  credentials: {
    JIRA_BASE_URL: "https://example.atlassian.net",
    JIRA_EMAIL: "dev@example.com",
    JIRA_API_TOKEN: "jira-test",
  },
  probe: { args: [], exitCode: 1, output: "plan         init|run|status|path" },
  notice: {
    claude:
      "jira is deprecated and will be removed in v8.0.0; uninstall with: claude plugin uninstall jira@toolu",
    codex:
      "jira is deprecated and will be removed in v8.0.0; uninstall with: codex plugin remove jira@toolu",
    opencode:
      "jira is deprecated and will be removed in v8.0.0; uninstall with: npx @toolu/plugins remove jira --host opencode --yes",
  },
});
