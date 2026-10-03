/**
 * SessionStart (#269): publish the agent-browser wrapper at
 * `<config root>/agent-browser/agent-browser.sh`, a stable path the agent's shell can expand
 * (`CLAUDE_PLUGIN_ROOT` reaches hook processes only). Silent on success;
 * every failure is non-fatal. Port of the bash `session-start.sh`.
 */
import { resolve } from "node:path";
import { publishBunCli, renderHookOutput, sessionContext } from "@toolu/core/startup";

const published = publishBunCli({
  plugin: "agent-browser",
  source: resolve(import.meta.dir, "../dist/agent-browser.js"),
  dir: "agent-browser",
  name: "agent-browser.sh",
  tool: "agent-browser wrapper",
});

if (process.env.TOOLU_HOST_OVERRIDE === "opencode" && published.status === "published") {
  const context = sessionContext(
    "SessionStart",
    `agent-browser is ready at ${published.path}. Load the native skill agent-browser-agent-browser when driving a real browser. Use the helper to open the page, snapshot its accessibility tree, act on an @eN ref, re-snapshot after changes, then close. The external agent-browser binary and Chromium are prerequisites; if missing, follow the helper's install diagnostic.`,
  );
  if (context !== undefined) process.stdout.write(renderHookOutput(context, false));
}
