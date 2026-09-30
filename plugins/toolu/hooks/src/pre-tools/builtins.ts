/**
 * The toolu plugin's built-in PreToolUse modules, in the byte order
 * `pre-tools/mod.sh` globbed `modules/*.sh`. Every one is a native gate from
 * `@toolu/core/gates` (#260, #261, #262); the dispatcher runs no built-in bash.
 */
import { dirname } from "node:path";
import type { ToolModule } from "@toolu/core/dispatch";
import {
  bashCommandsModule,
  codeEditRulesModule,
  commitGateModule,
  docsSyncModule,
  mcpBlockerModule,
  planLedgerModule,
  protectedFilesModule,
  pushReviewModule,
  qualityGateModule,
  type GateModuleOptions,
} from "@toolu/core/gates";

/** Each built-in by name, in dispatch order. */
export const NATIVE_MODULES = {
  "bash-commands": bashCommandsModule,
  "code-edit-rules": codeEditRulesModule,
  "commit-gate": commitGateModule,
  "docs-sync": docsSyncModule,
  "mcp-blocker": mcpBlockerModule,
  "plan-ledger": planLedgerModule,
  "protected-files": protectedFilesModule,
  "push-review": pushReviewModule,
  "quality-gate": qualityGateModule,
} as const satisfies Record<string, (options: GateModuleOptions) => ToolModule>;

export const BUILTIN_MODULES = Object.keys(NATIVE_MODULES);

/** The dispatch table for the plugin whose `hooks/` directory is `hooksDir`. */
export function builtins(hooksDir: string): ToolModule[] {
  const options = { pluginRoot: dirname(hooksDir) };
  return Object.values(NATIVE_MODULES).map((module) => module(options));
}
