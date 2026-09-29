/**
 * SessionStart (#269): publish the Context7 search CLI at
 * `<config root>/context7/search.sh`, a stable path the agent's shell can expand
 * (`CLAUDE_PLUGIN_ROOT` reaches hook processes only). Silent on success;
 * every failure is non-fatal. Port of the bash `session-start.sh`.
 */
import { resolve } from "node:path";
import { publishBunCli } from "@toolu/core/startup";

publishBunCli({
  plugin: "context7",
  source: resolve(import.meta.dir, "../dist/search.js"),
  dir: "context7",
  name: "search.sh",
  tool: "context7 search CLI",
});
