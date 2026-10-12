//! Start, keep, replace or resume the worker (`launch/agent.ts`): a live agent
//! must be the recorded one, a resume must attach the recorded session, and a
//! host change needs `--replace`.

use super::checkout::shell_join;
use super::failure::Failure;
use super::lifecycle::{Expected, Herdr, Outcome, inspect_agent, require_identity, shutdown_agent};
use super::session::require_captured_session;
use crate::herdr::Call;
use crate::hosts::{AgentOpts, HostKind, agent_args, host_kind};

/// The host, model and effort a launch runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved {
  /// The host.
  pub kind: HostKind,
  /// The model, when routed or given.
  pub model: Option<String>,
  /// The effort, when routed or given.
  pub effort: Option<String>,
}

/// What the agent step knows about this issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentPlan {
  /// The herdr agent name (the issue key).
  pub key: String,
  /// The pane the agent runs in.
  pub pane: String,
  /// Print the start without running it.
  pub dry: bool,
  /// `--replace`.
  pub replace: bool,
  /// The host to run.
  pub host: Resolved,
  /// Approvals off.
  pub bypass: bool,
  /// Claude's permission mode when attended.
  pub permission_mode: String,
  /// The record's previous host.
  pub previous_kind: Option<String>,
  /// Successful agent starts so far.
  pub launches: u64,
  /// The worktree.
  pub worktree: String,
  /// The captured session.
  pub session_id: Option<String>,
  /// When the captured session's attempt began (epoch ms).
  pub session_started_after: Option<u64>,
}

/// How the agent step ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentStart {
  /// An agent was started.
  pub started: bool,
  /// It resumed the captured session.
  pub resumed: bool,
  /// The argv after `--` in `agent start`.
  pub args: Vec<String>,
}

fn require_recorded_session(
  herdr: &Herdr<'_>,
  kind: HostKind,
  plan: &AgentPlan,
) -> Result<(), Failure> {
  let (Some(id), Some(since)) = (plan.session_id.as_deref(), plan.session_started_after) else {
    return Err(Failure::refusal(
      "live session identity is missing; inspect before explicit --replace",
    ));
  };
  require_captured_session(kind, &plan.worktree, since, id, herdr.env).map_err(|_err| {
    Failure::refusal(
      "live session identity is missing or changed; inspect before explicit --replace",
    )
  })
}

fn previous(plan: &AgentPlan) -> Result<HostKind, Failure> {
  plan
    .previous_kind
    .as_deref()
    .map_or(Ok(plan.host.kind), |kind| {
      host_kind(kind).map_err(Failure::refusal)
    })
}

/// A verified live agent: kept (`Some(start)`), or stopped for `--replace`.
fn keep_or_stop(
  herdr: &mut Herdr<'_>,
  plan: &AgentPlan,
  log: &mut Vec<String>,
) -> Result<Option<AgentStart>, Failure> {
  let (key, kind) = (plan.key.as_str(), plan.host.kind);
  let Some(live) = inspect_agent(herdr, key)? else {
    return Ok(None);
  };
  let expected = Expected {
    kind: previous(plan)?,
    pane: &plan.pane,
    cwd: &plan.worktree,
  };
  require_identity(key, &live, &expected)?;
  let moving = plan
    .previous_kind
    .as_deref()
    .is_some_and(|previous| previous != kind.name());
  if moving && !plan.replace {
    return Err(Failure::refusal("host change requires explicit --replace"));
  }
  if !plan.replace {
    require_recorded_session(herdr, kind, plan)?;
    log.push(format!("agent {key} already live"));
    return Ok(Some(AgentStart {
      started: false,
      resumed: false,
      args: Vec::new(),
    }));
  }
  log.push(format!("herdr agent prompt {key} /exit"));
  if shutdown_agent(herdr, key, &expected)? != Outcome::Exited {
    return Err(Failure::refusal(format!(
      "agent {key} did not exit; reconcile before --replace"
    )));
  }
  Ok(None)
}

/// The `agent start` call and its argv after `--`.
fn start_call(plan: &AgentPlan, resume: bool) -> Result<(Call, Vec<String>), Failure> {
  let kind = plan.host.kind;
  let opts = AgentOpts {
    key: plan.key.clone(),
    model: plan.host.model.clone(),
    effort: plan.host.effort.clone(),
    bypass: plan.bypass,
    permission_mode: plan.permission_mode.clone(),
    resume,
    session_id: plan.session_id.clone(),
  };
  let args = agent_args(kind, &opts).map_err(Failure::refusal)?;
  let start = Call::Start {
    name: plan.key.clone(),
    kind: kind.name().to_owned(),
    pane_id: plan.pane.clone(),
    args: args.clone(),
    timeout_ms: Some(90_000),
  };
  Ok((start, args))
}

/// Keep a verified live agent, stop it for `--replace`, then start or resume.
///
/// # Errors
/// A foreign live agent, a host change without replace, a missing session,
/// an agent that will not exit, or herdr failing.
pub(crate) fn ensure_agent(
  herdr: &mut Herdr<'_>,
  plan: &AgentPlan,
  log: &mut Vec<String>,
) -> Result<AgentStart, Failure> {
  let (key, kind) = (plan.key.as_str(), plan.host.kind);
  if !plan.dry
    && let Some(kept) = keep_or_stop(herdr, plan, log)?
  {
    return Ok(kept);
  }
  let resume =
    plan.launches > 0 && plan.previous_kind.as_deref() == Some(kind.name()) && !plan.replace;
  if resume && plan.session_id.is_none() {
    return Err(Failure::refusal(
      "captured session ID missing; reconcile the previous launch or explicitly --replace",
    ));
  }
  let (start, args) = start_call(plan, resume)?;
  log.push(shell_join(&start.argv(herdr.env)));
  if plan.dry {
    return Ok(AgentStart {
      started: true,
      resumed: resume,
      args,
    });
  }
  herdr.call(&start)?;
  if resume {
    let live = inspect_agent(herdr, key)?
      .ok_or_else(|| Failure::refusal("resumed agent disappeared before identity verification"))?;
    require_identity(
      key,
      &live,
      &Expected {
        kind,
        pane: &plan.pane,
        cwd: &plan.worktree,
      },
    )?;
    require_recorded_session(herdr, kind, plan)?;
  }
  Ok(AgentStart {
    started: true,
    resumed: resume,
    args,
  })
}

#[cfg(test)]
#[path = "tests/agent_test.rs"]
mod tests;
