/**
 * SessionStart (#269): publish the Context7 search CLI at
 * `<config root>/context7/search.sh`, a stable path the agent's shell can expand
 * (`CLAUDE_PLUGIN_ROOT` reaches hook processes only). Silent on success;
 * every failure is non-fatal. Port of the bash `session-start.sh`.
 * On OpenCode it also gives the documentation-first instruction, except on
 * compaction: the system transform already carries it on every request (#348).
 */
import { lstatSync } from "node:fs";
import { resolve } from "node:path";
import {
  publishBunCli,
  publishWrapper,
  renderHookOutput,
  sessionContext,
} from "@toolu/core/startup";
import { command, compacting, instruction, onOpencode } from "./context7/opencode.ts";

const options = {
  plugin: "context7",
  source: resolve(import.meta.dir, "../dist/search.js"),
  dir: "context7",
  name: "search.sh",
};

if (onOpencode()) {
  // The instruction names Bun by absolute path, so a PATH advisory would be false.
  const quiet = await compacting();
  const result = publishWrapper(options);
  if (!quiet && (result.status === "published" || result.status === "kept-user-file")) {
    const symlink = lstatSync(result.path).isSymbolicLink();
    process.stdout.write(
      renderHookOutput(
        sessionContext("SessionStart", instruction(command(result.path, symlink))),
        false,
      ),
    );
  }
} else {
  publishBunCli({ ...options, tool: "context7 search CLI" });
}
