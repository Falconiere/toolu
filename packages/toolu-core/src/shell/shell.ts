/**
 * `@toolu/core/shell` (#284): parse a Bash/Shell command once with unbash and
 * answer what runs, which git operation it performs, and whether its exit
 * status proves a command passed. What it writes is `writeTargets` in the
 * `@toolu/core/shell/writes` entry, kept apart so a bundle that does not need it
 * stays small. No gate reads a command as text. See docs/shell-analysis.md.
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
