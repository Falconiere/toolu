/**
 * SessionStart (#269): publish the Context7 search CLI at
 * `<config root>/context7/search.sh`, a stable path the agent's shell can expand
 * (`CLAUDE_PLUGIN_ROOT` reaches hook processes only). Prints the deprecation
 * notice (#403) and is otherwise silent on success; every failure is
 * non-fatal. Port of the bash `session-start.sh`.
 * On OpenCode it also gives the documentation-first instruction, except on
 * compaction: the system transform already carries it on every request (#348),
 * and the notice already showed at start.
 */
import { lstatSync } from "node:fs";
import { resolve } from "node:path";
import {
  deprecatedStartupOutput,
  publishBunCli,
  publishWrapper,
  sessionContext,
  type SessionContext,
} from "@toolu/core/startup";
import { command, compacting, instruction, onOpencode } from "./context7/opencode.ts";

const options = {
  plugin: "context7",
  source: resolve(import.meta.dir, "../dist/search.js"),
  dir: "context7",
  name: "search.sh",
};

let context: SessionContext | undefined;
let quiet = false;
if (onOpencode()) {
  // The instruction names Bun by absolute path, so a PATH advisory would be false.
  quiet = await compacting();
  const result = publishWrapper(options);
  if (!quiet && (result.status === "published" || result.status === "kept-user-file")) {
    const symlink = lstatSync(result.path).isSymbolicLink();
    context = sessionContext("SessionStart", instruction(command(result.path, symlink)));
  }
} else {
  publishBunCli({ ...options, tool: "context7 search CLI" });
}
process.stdout.write(deprecatedStartupOutput("context7", context, { compacting: quiet }));
