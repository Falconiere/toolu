/**
 * `@toolu/core/detect` (#254): the answers `plugins/toolu/hooks/lib/detect.sh`
 * gives, with the same results on the same inputs: project markers and
 * linters, tool availability, code-line counts, branch naming, and which
 * repository and branch a `git push` targets. Git questions take a
 * `ShellAnalysis` from `@toolu/core/shell`, so importing this entry loads
 * neither the shell parser nor zod. See docs/detect.md.
 */
export { baseBranch, branchSlug } from "./detect-branch.ts";
export { isGitCommit, isGitPush, pushTargetBranch, pushTargetRoot } from "./detect-git.ts";
export { countCodeLines, countPythonCodeLines, hasUnterminatedBlock } from "./detect-lines.ts";
export {
  detectClippy,
  detectPython,
  detectRust,
  detectTs,
  nodePackageManager,
  projectName,
  projectToplevel,
  pythonLinter,
  toRelativePath,
  tsLinter,
  type DetectOptions,
  type NodePackageManager,
  type TsLinter,
} from "./detect-project.ts";
export { detectAstGrep, toolAvailable } from "./detect-tools.ts";
