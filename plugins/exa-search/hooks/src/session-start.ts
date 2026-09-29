/**
 * SessionStart (#269): publish the Exa search CLI at
 * `<config root>/exa-search/search.sh`, a stable path the agent's shell can expand
 * (`CLAUDE_PLUGIN_ROOT` reaches hook processes only). Silent on success;
 * every failure is non-fatal. Port of the bash `session-start.sh`.
 */
import { resolve } from "node:path";
import { publishBunCli } from "@toolu/core/startup";

publishBunCli({
  plugin: "exa-search",
  source: resolve(import.meta.dir, "../dist/search.js"),
  dir: "exa-search",
  name: "search.sh",
  tool: "exa-search search CLI",
});
