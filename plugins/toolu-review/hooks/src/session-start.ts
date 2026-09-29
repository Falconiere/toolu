/**
 * SessionStart (#269): publish the push-review state writer at
 * `<config root>/toolu-review/write-state.sh`, a stable path the agent's shell can expand
 * (`CLAUDE_PLUGIN_ROOT` reaches hook processes only). Silent on success;
 * every failure is non-fatal. Port of the bash `session-start.sh`.
 */
import { resolve } from "node:path";
import { publishBunCli } from "@toolu/core/startup";

publishBunCli({
  plugin: "toolu-review",
  source: resolve(import.meta.dir, "../dist/write-state.js"),
  dir: "toolu-review",
  name: "write-state.sh",
  what: "helper",
  tool: "toolu-review write-state helper",
});
