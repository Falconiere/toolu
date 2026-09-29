/** agent-browser's SessionStart bundle through its real hooks.json launcher (#269, ported from session-start.bats). */
import { resolve } from "node:path";
import { publishedCliSuite } from "@toolu/conformance/harness/startup";

publishedCliSuite({
  plugin: "agent-browser",
  pluginRoot: resolve(import.meta.dir, "../../.."),
  source: "hooks/dist/agent-browser.js",
  dir: "agent-browser",
  name: "agent-browser.sh",
  advisory:
    "agent-browser: bun not found on PATH — the agent-browser wrapper needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)",
  credentials: {},
  probe: {
    args: ["snapshot"],
    env: { AGENT_BROWSER_BIN: "/nonexistent/agent-browser" },
    exitCode: 127,
    output: "agent-browser not found",
  },
});
