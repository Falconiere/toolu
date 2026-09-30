/**
 * `@toolu/core/gates`: built-in toolu gates as native modules, and what they
 * read from a hook payload. The PreToolUse gates (`bash-commands`,
 * `commit-gate`, `quality-gate`, #261; `protected-files`, `mcp-blocker`,
 * `code-edit-rules`, #260; `push-review`, `plan-ledger`, `docs-sync`, #262) and the PostToolUse gates (`gate-status`,
 * `push-waiver`, #259) run in `@toolu/core/dispatch`'s walk like any
 * `ToolModule`. `@toolu/core/gates/mcp-hook` is the standalone `mcp__` hook on
 * its own entry, so its bundle does not carry the shell parser, and loads the
 * gate only when a call needs it.
 */
export {
  bashCommandsDecide,
  bashCommandsModule,
  type BashCommandsVerdict,
  type BashLists,
} from "./bash-commands.ts";
export { commitGateModule, commitPrefix } from "./commit-gate.ts";
export type { GateModule, GateModuleOptions } from "./gate-module.ts";
export { qualityGateModule } from "./quality-gate.ts";
export { qualityCommands, type QualityCommand } from "./quality-command.ts";
export { toolCommand, toolExitStatus, toolInterrupted } from "./tool-exit.ts";
export { gateStatusModule } from "./gate-status.ts";
export { pushWaiverModule } from "./push-waiver.ts";
export { bashPatternMatch, compileBashPattern, type BashPattern } from "./bash-pattern.ts";
export { codeEditRulesModule } from "./code-edit-rules.ts";
export { expandPattern, repoRelative } from "./gate-paths.ts";
export { mcpBlockerModule } from "./mcp-blocker.ts";
export { protectedFilesModule } from "./protected-files.ts";
export { docsSyncModule } from "./docs-sync.ts";
export { planLedgerModule } from "./plan-ledger.ts";
export { pushReviewModule } from "./push-review.ts";
