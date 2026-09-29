/**
 * The toolu plugin's built-in PostToolUse modules (#259), in the byte order
 * `post-tools/mod.sh` globs `modules/*.sh`. Both are native ports; the bash
 * scripts stay as the parity baseline until the OpenCode bridge drops them.
 */
import type { ToolModule } from "@toolu/core/dispatch";
import { gateStatusModule, pushWaiverModule } from "@toolu/core/gates";

export const BUILTIN_MODULES = ["gate-status", "push-waiver"] as const;

/** The dispatch table, one module per `BUILTIN_MODULES` entry. */
export function builtins(): ToolModule[] {
  return [gateStatusModule, pushWaiverModule];
}
