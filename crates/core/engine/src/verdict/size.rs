//! The verdict's size gate: the pr-size push gate's measurement, read-only.

use toolu_protocol::event::HostEvent;
use toolu_runtime::config::gate_mode::{GateMode, gate_mode};
use toolu_runtime::config::load::load;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::git::base_branch;

use super::gates::{GateContext, gate};
use crate::gates::pr_size::branch_size;

/// The branch's code lines and changed files against the `gates.prSize` budgets.
pub fn size_gate(ctx: &mut GateContext<'_>) -> Ordered {
  let config = load(ctx.roots, Some(&ctx.cwd));
  let host = Some(ctx.roots.host());
  let mode = gate_mode(&config, "prSize", host, Some(HostEvent::ShellPre));
  // The docs gate loaded the same files; repeat none of its warnings.
  for warning in config.take_warnings() {
    if !ctx.warnings.contains(&warning) {
      ctx.warnings.push(warning);
    }
  }
  let env = ctx.roots.env();
  if mode == GateMode::Off {
    return gate("skip", "prSize mode is off", Vec::new());
  }
  if ctx.branch == base_branch(env, Some(&ctx.root), &ctx.root) {
    return gate("skip", "current branch is the base branch", Vec::new());
  }
  let Some(size) = branch_size(&ctx.root, env, &config) else {
    return gate("skip", "no diff to measure", Vec::new());
  };
  let reason = size.summary();
  let state = match (size.over(), mode) {
    (false, _) => "pass",
    (true, GateMode::Block | GateMode::Ask) => "fail",
    (true, _) => "advise",
  };
  gate(state, &reason, Vec::new())
}

#[cfg(test)]
#[path = "tests/size_test.rs"]
mod tests;
