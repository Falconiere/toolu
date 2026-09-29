/**
 * PostToolUse for edits, shell and search tools (#259): the TypeScript
 * dispatcher over the native gate-status and push-waiver modules, then the
 * `post-tools.d` registry (language-quality modules on bash until #265–#267).
 * Replaces `post-tools/mod.sh` in hooks.json; `mod.sh` stays as the parity
 * baseline.
 */
import { join } from "node:path";
import { dispatchPostTool } from "@toolu/core/dispatch";
import { builtins } from "./post-tools/builtins.ts";
import { hookMain } from "./pre-tools/hook-main.ts";

await hookMain(
  import.meta.dir,
  (stdin, hooks) => dispatchPostTool(stdin, { builtins: builtins(), libDir: join(hooks, "lib") }),
  "PostToolUse",
);
