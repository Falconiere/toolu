//! Start, keep, replace or resume the worker (`launch/agent.ts`): a live agent
//! must be the recorded one, a resume must attach the recorded session, and a
//! host change needs `--replace`.

use std::time::{Duration, Instant};

use super::checkout::shell_join;
use super::failure::Failure;
use super::lifecycle::{
  Expected, Herdr, Outcome, await_named_agent, inspect_agent, require_identity, shutdown_agent,
};
use super::session::{capture_session, require_captured_session};
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
  /// How long `agent start` is retried while the pane's shell is not ready.
  pub shell_wait: Duration,
}

/// The first and the longest pause between `agent start` retries.
const BACKOFF: (Duration, Duration) = (Duration::from_millis(250), Duration::from_secs(2));

/// How the agent step ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentStart {
  /// An agent was started.
  pub started: bool,
  /// It resumed the captured session.
  pub resumed: bool,
  /// The argv after `--` in `agent start`.
  pub args: Vec<String>,
  /// A session the record lacked, captured from the host's store for the
  /// kept agent (#625).
  pub captured: Option<String>,
}

/// The session a live agent runs: the recorded one, which the host's own
/// records must still name (`None`), or, when the launch never captured it
/// (#625), the one session the host wrote for the worktree since the attempt
/// began (`Some`).
fn recorded_session(
  herdr: &Herdr<'_>,
  kind: HostKind,
  plan: &AgentPlan,
) -> Result<Option<String>, Failure> {
  let missing =
    || Failure::refusal("live session identity is missing; inspect before explicit --replace");
  let Some(since) = plan.session_started_after else {
    return Err(missing());
  };
  let Some(id) = plan.session_id.as_deref() else {
    let captured = capture_session(kind, &plan.worktree, since, herdr.env)?;
    return captured.map(Some).ok_or_else(missing);
  };
  require_captured_session(kind, &plan.worktree, since, id, herdr.env).map_err(|_err| {
    Failure::refusal(
      "live session identity is missing or changed; inspect before explicit --replace",
    )
  })?;
  Ok(None)
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
    let captured = recorded_session(herdr, kind, plan)?;
    log.push(format!("agent {key} already live"));
    return Ok(Some(AgentStart {
      started: false,
      resumed: false,
      args: Vec::new(),
      captured,
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

/// Wait for a fresh start to answer to its name before its first prompt.
///
/// # Errors
/// The agent stays unaddressable, or herdr fails.
fn await_start(
  herdr: &mut Herdr<'_>,
  plan: &AgentPlan,
  kind: HostKind,
  wait: Duration,
) -> Result<(), Failure> {
  await_named_agent(
    herdr,
    &plan.key,
    &Expected {
      kind,
      pane: &plan.pane,
      cwd: &plan.worktree,
    },
    wait,
  )
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

/// Send `agent start`, and send it again while herdr answers that the pane's
/// shell is not at its prompt yet: herdr refused before starting anything, so
/// a resend cannot start a second agent. Pauses double from 250 ms to 2 s and
/// never run past `plan.shell_wait`; then the last refusal is returned.
///
/// # Errors
/// herdr fails, or still finds the pane busy when the wait is spent.
fn send_start(
  herdr: &mut Herdr<'_>,
  start: &Call,
  plan: &AgentPlan,
  log: &mut Vec<String>,
) -> Result<(), Failure> {
  let began = Instant::now();
  let mut pause = BACKOFF.0;
  loop {
    let err = match herdr.call(start) {
      Ok(_) => return Ok(()),
      Err(err) if err.not_started() => err,
      Err(err) => return Err(err),
    };
    let left = plan.shell_wait.saturating_sub(began.elapsed());
    if left.is_zero() {
      return Err(err);
    }
    let sleep = pause.min(left);
    log.push(format!(
      "herdr agent.start: pane {} busy; retry in {} ms",
      plan.pane,
      sleep.as_millis()
    ));
    std::thread::sleep(sleep);
    pause = pause.saturating_mul(2).min(BACKOFF.1);
  }
}

/// After `agent start`: a fresh agent must answer to its name, a resumed one
/// must keep its recorded identity and session.
fn verify_started(
  herdr: &mut Herdr<'_>,
  plan: &AgentPlan,
  resume: bool,
  wait: Duration,
) -> Result<(), Failure> {
  let (key, kind) = (plan.key.as_str(), plan.host.kind);
  if !resume {
    return await_start(herdr, plan, kind, wait);
  }
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
  recorded_session(herdr, kind, plan).map(|_resumed| ())
}

/// Keep a verified live agent, stop it for `--replace`, then start or resume.
/// A fresh start waits up to `wait` for herdr to name the agent, so the
/// brief never sends to an agent that cannot receive it.
///
/// # Errors
/// A foreign live agent, a host change without replace, a missing session,
/// an agent that will not exit, an agent that stays unaddressable, or herdr
/// failing.
pub(crate) fn ensure_agent(
  herdr: &mut Herdr<'_>,
  plan: &AgentPlan,
  log: &mut Vec<String>,
  wait: Duration,
) -> Result<AgentStart, Failure> {
  let kind = plan.host.kind;
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
      captured: None,
    });
  }
  send_start(herdr, &start, plan, log)?;
  verify_started(herdr, plan, resume, wait)?;
  Ok(AgentStart {
    started: true,
    resumed: resume,
    args,
    captured: None,
  })
}

#[cfg(test)]
#[path = "tests/agent_test.rs"]
mod tests;
