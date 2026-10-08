//! The repository and branch a parsed `git push` targets.

use std::path::{Path, PathBuf};

use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::process::{Spec, run};
use toolu_runtime::registry::rule::RuleContext;
use toolu_shell::analysis::ShellAnalysis;
use toolu_state::git::has_git;

use super::command_analysis;
use crate::detect::{is_git_push, push_target_branch, push_target_root};

/// The checked-out branch, or a detached push's refspec destination.
pub(crate) struct PushTarget {
  pub(crate) root: PathBuf,
  pub(crate) branch: String,
}

/// A parsed push, or none for another tool, unknown command, or absent git.
pub(crate) fn push_target(event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Option<PushTarget> {
  if !matches!(event, NormalizedEvent::ShellPre { .. }) {
    return None;
  }
  let analysis: ShellAnalysis = command_analysis(ctx);
  if !is_git_push(&analysis) || !has_git(ctx.env) {
    return None;
  }
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  let cwd = ctx.cwd.unwrap_or(ctx.project_root);
  let root = push_target_root(&analysis, &roots, cwd);
  let branch = push_target_branch(&analysis, &root, ctx.env);
  Some(PushTarget { root, branch })
}

/// `git -C root args...` stdout on success.
pub(crate) fn git_at(root: &Path, args: &[&str], env: &Env) -> Option<String> {
  let mut spec = Spec::new(["git", "-C", &root.display().to_string()]);
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  spec.env = Some(env.clone());
  spec.max_output_bytes = usize::MAX;
  run(&spec)
    .ok()
    .filter(|out| out.exit_code == 0)
    .map(|out| out.stdout)
}

/// Whether a local ref exists in the target repository.
pub(crate) fn ref_exists(root: &Path, reference: &str, env: &Env) -> bool {
  git_at(root, &["rev-parse", "--verify", "--quiet", reference], env).is_some()
}

/// Changed paths as Git prints them, or empty if Git could not compute them.
pub(crate) fn changed_names(root: &Path, base: &str, env: &Env) -> String {
  git_at(
    root,
    &[
      "diff",
      "--no-color",
      &format!("{base}...HEAD"),
      "--name-only",
    ],
    env,
  )
  .unwrap_or_default()
}

#[cfg(test)]
#[path = "tests/push_target_test.rs"]
mod tests;
