/**
 * SessionStart (#269): publish the Jira CLI at
 * `<config root>/jira/jira.sh`, a stable path the agent's shell can expand
 * (`CLAUDE_PLUGIN_ROOT` reaches hook processes only). Silent on success;
 * every failure is non-fatal. Port of the bash `session-start.sh`.
 */
import { resolve } from "node:path";
import { publishBunCli } from "@toolu/core/startup";

publishBunCli({
  plugin: "jira",
  source: resolve(import.meta.dir, "../dist/jira.js"),
  dir: "jira",
  name: "jira.sh",
  tool: "jira CLI",
});
