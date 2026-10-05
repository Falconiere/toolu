/**
 * SessionStart (#269): publish the agent-browser wrapper at
 * `<config root>/agent-browser/agent-browser.sh`, a stable path the agent's shell can expand
 * (`CLAUDE_PLUGIN_ROOT` reaches hook processes only). Prints the deprecation
 * notice (#403), plus the helper instruction on OpenCode, where the notice
 * shows once per start, not per compaction. Every failure is non-fatal.
 * Port of the bash `session-start.sh`.
 */
import { resolve } from "node:path";
import {
  deprecatedStartupOutput,
  publishBunCli,
  sessionContext,
  startedByCompaction,
  type SessionContext,
} from "@toolu/core/startup";

const published = publishBunCli({
  plugin: "agent-browser",
  source: resolve(import.meta.dir, "../dist/agent-browser.js"),
  dir: "agent-browser",
  name: "agent-browser.sh",
  tool: "agent-browser wrapper",
});

const onOpencode = process.env.TOOLU_HOST_OVERRIDE === "opencode";
let context: SessionContext | undefined;
if (onOpencode && published.status === "published") {
  context = sessionContext(
    "SessionStart",
    `agent-browser is ready at ${published.path}. Load the native skill agent-browser-agent-browser when driving a real browser. Use the helper to open the page, snapshot its accessibility tree, act on an @eN ref, re-snapshot after changes, then close. The external agent-browser binary and Chromium are prerequisites; if missing, follow the helper's install diagnostic.`,
  );
}
const compacting = onOpencode && (await startedByCompaction());
process.stdout.write(deprecatedStartupOutput("agent-browser", context, { compacting }));
