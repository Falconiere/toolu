//! PostToolUse push-waiver (`push-waiver.ts`, the native port of
//! `post-tools/modules/push-waiver.sh`): the push-review gate can only ask, and
//! a PostToolUse event for the push is the "yes". A successful, uninterrupted
//! push promotes the pending marker for exactly the diff that was asked about
//! (`toolu_state::push_waiver::push_waiver_promote`); a failed push leaves it
//! for the retry. Pushes are detected from the parsed command (wrappers,
//! `bash -c`, `eval`, git by path and global options all count), and the pushed
//! repository is `crate::detect::push_target_root`. It never writes stdout.

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::ctx::StateCtx;
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::{base_branch, branch_slug, current_branch};
use toolu_state::push_waiver::push_waiver_promote;

use super::tool_exit::{tool_exit_status, tool_interrupted};
use super::{command_analysis, is_shell_tool};
use crate::detect::{is_git_push, push_target_root};
use crate::gate::Gate;

/// The built-in `push-waiver.sh`.
#[derive(Debug, Clone, Copy)]
pub struct PushWaiver;

/// The one `push-waiver.sh` built-in.
pub static PUSH_WAIVER: PushWaiver = PushWaiver;

/// A reported exit status other than 0 means the push did not land.
fn push_failed(status: &str) -> bool {
  !matches!(status, "" | "null" | "0")
}

impl Gate for PushWaiver {
  fn name(&self) -> &'static str {
    "push-waiver.sh"
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String> {
    if !is_shell_tool(event) {
      return Ok(Decision::Allow);
    }
    let analysis = command_analysis(ctx);
    if !is_git_push(&analysis)
      || push_failed(&tool_exit_status(ctx.raw))
      || tool_interrupted(ctx.raw)
    {
      return Ok(Decision::Allow);
    }
    let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
    let cwd = ctx.cwd.unwrap_or(ctx.project_root);
    let root = push_target_root(&analysis, &roots, cwd);
    // Without git there is no branch either (bash: `command -v git || exit 0`).
    let branch = current_branch(ctx.env, &root);
    if branch.is_empty() || branch == "HEAD" {
      return Ok(Decision::Allow);
    }
    let base = ctx
      .env
      .get("PUSH_REVIEW_BASE")
      .map_or_else(|| base_branch(ctx.env, Some(&root), cwd), str::to_owned);
    if let Some(sha) = diff_sha(ctx.env, &root, &base) {
      push_waiver_promote(&StateCtx::new(roots), &root, &branch_slug(&branch), &sha);
    }
    Ok(Decision::Allow)
  }
}

#[cfg(test)]
#[path = "tests/push_waiver_test.rs"]
mod tests;
