/**
 * `@toolu/core/shell` (#284): parse a Bash/Shell command once with unbash and
 * answer what runs, what it writes, which git operation it performs, and
 * whether its exit status proves a command passed. No gate reads a command as
 * text. See docs/shell-analysis.md for the contract and policy.
 */
export { MAX_SHELL_INPUT, analyzeShell } from "./shell-parse.ts";
export { MAX_RUN_DEPTH } from "./shell-walk.ts";
export { shellAnalysisOf, type ShellPreEvent } from "./shell-event.ts";
export {
  commitMessages,
  gitInvocation,
  pushTargets,
  runsGitSubcommand,
  type GitInvocation,
  type GitPush,
} from "./shell-git.ts";
export { writeTargets, type WriteTarget, type WriteVia } from "./shell-writes.ts";
export { matchesRule } from "./shell-rules.ts";
export type {
  CommandOrigin,
  PipelinePosition,
  ShellAnalysis,
  ShellCommand,
  ShellError,
  ShellRedirect,
  Tristate,
} from "./shell-types.ts";
