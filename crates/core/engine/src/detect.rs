//! Git operations a command performs (`detect-git.ts`, the ports of
//! `is_git_push`, `is_git_commit`, `push_target_root` and `push_target_branch`),
//! answered from the command's `ShellAnalysis` rather than its text. Gates parse
//! once per event and pass the analysis in. Git facts come from `.git`; git is
//! spawned only where `toolu_runtime::git` defers to it.

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;
use toolu_runtime::git::toplevel;
use toolu_runtime::host::roots::Roots;
use toolu_shell::analysis::{ShellAnalysis, Tristate};
use toolu_shell::git::{push_targets, runs_git_subcommand};
use toolu_state::git::current_branch;

/// Whether the command runs `git push`. A dynamic name (`$g push`) is unknown
/// to the shell layer and false here, as bash answers for the workflow gates; a
/// gate that must fail closed asks `runs_git_subcommand` itself.
pub fn is_git_push(analysis: &ShellAnalysis) -> bool {
  runs_git_subcommand(analysis, "push") == Tristate::Yes
}

/// Whether the command runs `git commit`; unknown is false, as for [`is_git_push`].
pub fn is_git_commit(analysis: &ShellAnalysis) -> bool {
  runs_git_subcommand(analysis, "commit") == Tristate::Yes
}

/// The toplevel of the repository the first push targets. Its whole `-C` chain
/// is replayed from `cwd`, each step relative to the last, as git applies them;
/// a dynamic step, or a directory with no repository, falls back to the cwd's
/// toplevel, then the host project root, then the cwd.
pub fn push_target_root(analysis: &ShellAnalysis, roots: &Roots, cwd: &Path) -> PathBuf {
  let env = roots.env();
  let pushes = push_targets(analysis);
  let chain = pushes
    .first()
    .map(|push| push.invocation.c_chain.as_slice())
    .unwrap_or_default();
  let replayed = (!chain.is_empty() && chain.iter().all(Option::is_some)).then(|| {
    chain
      .iter()
      .flatten()
      .fold(cwd.to_path_buf(), |dir, step| dir.join(step))
  });
  replayed
    .and_then(|dir| toplevel(env, &dir))
    .or_else(|| toplevel(env, cwd))
    .or_else(|| roots.project_root(Some(cwd)))
    .unwrap_or_else(|| cwd.to_path_buf())
}

/// The branch a push from `root` updates: the checked-out branch, or on a
/// detached or unborn HEAD the branch the first push's refspec names; `""` when
/// none can be named (a delete, bare `HEAD`, a wildcard, no or a dynamic refspec).
pub fn push_target_branch(analysis: &ShellAnalysis, root: &Path, env: &Env) -> String {
  let branch = current_branch(env, root);
  if !branch.is_empty() && branch != "HEAD" {
    return branch;
  }
  let pushes = push_targets(analysis);
  pushes
    .into_iter()
    .next()
    .and_then(|push| push.destination)
    .unwrap_or_default()
}

#[cfg(test)]
#[path = "tests/detect_test.rs"]
mod tests;
