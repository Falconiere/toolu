/**
 * The toolu plugin's built-in PreToolUse modules (#258), in the byte order
 * `pre-tools/mod.sh` globs `modules/*.sh`. Every module is still its bash
 * script; a port (#260–#262) swaps its `bashModule(...)` for the native module.
 */
import { join } from "node:path";
import { bashModule, type ToolModule } from "@toolu/core/dispatch";

export const BUILTIN_MODULES = [
  "bash-commands",
  "code-edit-rules",
  "commit-gate",
  "docs-sync",
  "mcp-blocker",
  "plan-ledger",
  "protected-files",
  "push-review",
  "quality-gate",
] as const;

/** The dispatch table for the plugin whose `hooks/` directory is `hooksDir`. */
export function builtins(hooksDir: string): ToolModule[] {
  const modules = join(hooksDir, "pre-tools", "modules");
  return BUILTIN_MODULES.map((name) => bashModule(modules, name));
}
