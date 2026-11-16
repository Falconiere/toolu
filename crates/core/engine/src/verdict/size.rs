//! The verdict's size gate: the pr-size push gate's measurement, read-only.

use std::path::Path;

use toolu_protocol::event::HostEvent;
use toolu_protocol::host::Host;
use toolu_runtime::config::gate_mode::{GateMode, gate_mode};
use toolu_runtime::config::load::load;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::git::base_branch;

use super::gates::{GateContext, gate};
use crate::gates::pr_size::{branch_size, rev_size};
use crate::gates::push_target::ref_exists;

/// The branch's code lines and changed files against the `gates.prSize` budgets.
pub fn size_gate(ctx: &mut GateContext<'_>) -> Ordered {
  let config = load(ctx.roots, Some(&ctx.cwd));
  let host = Some(ctx.roots.host());
  let mode = gate_mode(&config, "prSize", host, Some(HostEvent::ShellPre));
  let env = ctx.roots.env();
  let size = branch_size(&ctx.root, env, &config);
  // The docs gate loaded the same files; repeat none of its warnings.
  for warning in config.take_warnings() {
    if !ctx.warnings.contains(&warning) {
      ctx.warnings.push(warning);
    }
  }
  if mode == GateMode::Off {
    return gate("skip", "prSize mode is off", Vec::new());
  }
  if ctx.branch == base_branch(env, Some(&ctx.root), &ctx.root) {
    return gate("skip", "current branch is the base branch", Vec::new());
  }
  let (over, reason) = match size {
    Ok(Some(size)) => (size.over(), size.summary()),
    Ok(None) => return gate("skip", "git could not diff the branch", Vec::new()),
    Err(base) => (true, format!("base '{base}' not found locally")),
  };
  let state = match (over, mode) {
    (false, _) => "pass",
    (true, GateMode::Block | GateMode::Ask) => "fail",
    (true, _) => "advise",
  };
  gate(state, &reason, Vec::new())
}

/// Why the epic engine must not merge the pull request head `head`, measured
/// in the worktree at `root`: it is over its budget, or it or its base is
/// missing there, while the worktree's own config sets `gates.prSize` to
/// `block` or `ask`. `None` lets the merge go on. The head is measured, not
/// the checkout: a split that was never pushed must not pass for the head.
pub fn merge_refusal(root: &Path, env: &Env, head: &str) -> Option<String> {
  let roots = Roots::new(
    env
      .clone()
      .with("TOOLU_PROJECT_DIR", &root.to_string_lossy()),
    None,
  );
  let config = load(&roots, Some(root));
  // No human answers the engine, so `ask` keeps enforcing on any host.
  let mode = gate_mode(&config, "prSize", Some(Host::Claude), None);
  if !matches!(mode, GateMode::Block | GateMode::Ask) {
    return None;
  }
  if !ref_exists(root, &format!("{head}^{{commit}}"), env) {
    return Some(format!(
      "pr size unmeasured: head '{head}' not found in the worktree"
    ));
  }
  match rev_size(root, env, &config, head) {
    Ok(Some(size)) if size.over() => Some(format!("pr size over budget: {}", size.summary())),
    Ok(_) => None,
    Err(base) => Some(format!(
      "pr size unmeasured: base '{base}' not found locally"
    )),
  }
}

#[cfg(test)]
#[path = "tests/size_test.rs"]
mod tests;
