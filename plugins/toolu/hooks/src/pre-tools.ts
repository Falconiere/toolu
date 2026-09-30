/**
 * PreToolUse for edits, shell and search tools (#258): the TypeScript
 * dispatcher over the built-in modules, then the `pre-tools.d` registry.
 * Replaces `pre-tools/mod.sh` in hooks.json. Built-in modules are native;
 * their results are checked against golden captures of their Bash predecessors.
 */
import { join } from "node:path";
import { dispatchPreTool } from "@toolu/core/dispatch";
import { builtins } from "./pre-tools/builtins.ts";
import { hookMain } from "./pre-tools/hook-main.ts";

await hookMain(import.meta.dir, (stdin, hooks) =>
  dispatchPreTool(stdin, { builtins: builtins(hooks), libDir: join(hooks, "lib") }),
);
