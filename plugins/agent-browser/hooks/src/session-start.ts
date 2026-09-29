/**
 * SessionStart (#269): publish the agent-browser wrapper at
 * `<config root>/agent-browser/agent-browser.sh`, a stable path the agent's shell can expand
 * (`CLAUDE_PLUGIN_ROOT` reaches hook processes only). Silent on success;
 * every failure is non-fatal. Port of the bash `session-start.sh`.
 */
import { resolve } from "node:path";
import { publishBunCli } from "@toolu/core/startup";

publishBunCli({
  plugin: "agent-browser",
  source: resolve(import.meta.dir, "../dist/agent-browser.js"),
  dir: "agent-browser",
  name: "agent-browser.sh",
  tool: "agent-browser wrapper",
});
