//! Typed herdr observations of a worker (`lifecycle.ts`): its live identity,
//! whether a prompt was acknowledged, and a verified shutdown. A failed query
//! never means a worker has exited.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value;
use toolu_runtime::env::Env;

use super::failure::Failure;
use crate::herdr::{Call, Client};
use crate::herdr_calls::request;
use crate::hosts::{HostKind, host_kind, host_limited};
use crate::paths::Paths;
use crate::retry::number;
use crate::workload::{await_owned_exit, owned_workload};

/// herdr as the launch and finish verbs reach it.
pub(crate) struct Herdr<'a> {
  /// The protocol decision for this run.
  pub client: Client,
  /// The engine paths, for the CLI fallback's journal.
  pub paths: &'a Paths,
  /// The environment.
  pub env: &'a Env,
}

impl Herdr<'_> {
  /// One call's result.
  ///
  /// # Errors
  /// herdr fails or answers an error.
  pub(crate) fn call(&mut self, call: &Call) -> Result<Value, Failure> {
    request(&mut self.client, self.paths, self.env, call).map_err(Failure::from)
  }
}

/// How a lifecycle step ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
  /// A new agent started.
  Started,
  /// The previous conversation was resumed.
  Resumed,
  /// A live agent was kept.
  Retained,
  /// The prompt reached a blocked UI.
  Blocked,
  /// The provider refused for a usage limit.
  ProviderLimited,
  /// The agent and its workload are gone.
  Exited,
  /// Some workload remains.
  CleanupIncomplete,
}

impl Outcome {
  /// The record's `lifecycle_outcome`.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Self::Started => "started",
      Self::Resumed => "resumed",
      Self::Retained => "retained",
      Self::Blocked => "blocked",
      Self::ProviderLimited => "provider-limited",
      Self::Exited => "exited",
      Self::CleanupIncomplete => "cleanup-incomplete",
    }
  }
}

/// A live agent as herdr lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveAgent {
  /// The host it runs.
  pub kind: HostKind,
  /// Its pane.
  pub pane: String,
  /// Its cwd.
  pub cwd: String,
  /// herdr's status.
  pub status: String,
}

/// Who the agent should be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Expected<'a> {
  /// The host.
  pub kind: HostKind,
  /// The pane.
  pub pane: &'a str,
  /// The worktree.
  pub cwd: &'a str,
}

/// The live agent named `name`, if any.
///
/// # Errors
/// herdr fails, omits the list, or lists the agent without its identity.
pub(crate) fn inspect_agent(
  herdr: &mut Herdr<'_>,
  name: &str,
) -> Result<Option<LiveAgent>, Failure> {
  let reply = herdr.call(&Call::AgentList)?;
  let agents = reply
    .get("agents")
    .and_then(Value::as_array)
    .ok_or_else(|| Failure::refusal("herdr agent list omitted agents"))?;
  let Some(agent) = agents
    .iter()
    .find(|agent| agent.get("name").and_then(Value::as_str) == Some(name))
  else {
    return Ok(None);
  };
  let field = |key: &str| agent.get(key).and_then(Value::as_str).map(str::to_owned);
  let (Some(kind), Some(pane), Some(cwd)) = (field("agent"), field("pane_id"), field("cwd")) else {
    return Err(Failure::refusal(format!(
      "cannot verify live identity of {name}"
    )));
  };
  Ok(Some(LiveAgent {
    kind: host_kind(&kind).map_err(Failure::refusal)?,
    pane,
    cwd,
    status: field("agent_status").unwrap_or_else(|| "unknown".to_owned()),
  }))
}

fn real(path: &str) -> Result<std::path::PathBuf, Failure> {
  std::fs::canonicalize(path).map_err(|err| Failure::unknown(format!("{path}: {err}")))
}

/// Refuse an agent whose host, pane or worktree is not the recorded one.
///
/// # Errors
/// The identity differs, or a path cannot be resolved.
pub(crate) fn require_identity(
  name: &str,
  live: &LiveAgent,
  expected: &Expected<'_>,
) -> Result<(), Failure> {
  if live.kind != expected.kind
    || live.pane != expected.pane
    || real(&live.cwd)? != real(expected.cwd)?
  {
    return Err(Failure::refusal(format!(
      "live agent {name} identity does not match recorded host/pane/worktree"
    )));
  }
  Ok(())
}

/// A limit message in the tail wins; a blocked status is `blocked`.
pub(crate) fn acknowledged_status(status: &str, tail: &str, fallback: Outcome) -> Outcome {
  if host_limited(tail) {
    Outcome::ProviderLimited
  } else if status == "blocked" {
    Outcome::Blocked
  } else {
    fallback
  }
}

/// The agent is listed but herdr has not populated its identity yet: still
/// starting, not a failure.
fn identity_pending(name: &str, err: &Failure) -> bool {
  err.message == format!("cannot verify live identity of {name}")
}

/// Whether herdr no longer lists `name`. A dying agent's entry degrades
/// before its name disappears: still present, not a failure.
pub(crate) fn listing_gone(herdr: &mut Herdr<'_>, name: &str) -> Result<bool, Failure> {
  match inspect_agent(herdr, name) {
    Ok(agent) => Ok(agent.is_none()),
    Err(err) if identity_pending(name, &err) => Ok(false),
    Err(err) => Err(err),
  }
}

/// A status that accepts a prompt.
fn promptable(status: &str) -> bool {
  status == "idle" || status == "done"
}

/// Poll until herdr lists `name` with the recorded identity and a promptable
/// status, up to `wait`: a started agent precedes herdr naming it, and naming
/// precedes it accepting input. A prompt sent before then fails with
/// `agent_not_ready` while the agent idles with an empty prompt.
///
/// # Errors
/// The agent never appears promptable with its identity, or herdr fails.
pub(crate) fn await_named_agent(
  herdr: &mut Herdr<'_>,
  name: &str,
  expected: &Expected<'_>,
  wait: Duration,
) -> Result<(), Failure> {
  let deadline = Instant::now() + wait;
  loop {
    match inspect_agent(herdr, name) {
      Ok(Some(live)) => {
        require_identity(name, &live, expected)?;
        if promptable(&live.status) {
          return Ok(());
        }
      }
      Ok(None) => {}
      Err(err) if identity_pending(name, &err) => {}
      Err(err) => return Err(err),
    }
    if Instant::now() >= deadline {
      return Err(Failure::unknown(format!(
        "agent {name} is not addressable after start; inspect the owned pane before retry"
      )));
    }
    std::thread::sleep(Duration::from_millis(500));
  }
}

/// Whether a prompt is running work or stopped at a blocked UI.
///
/// # Errors
/// The agent disappeared, its identity changed, or herdr fails.
pub(crate) fn acknowledged_outcome(
  herdr: &mut Herdr<'_>,
  name: &str,
  expected: &Expected<'_>,
  fallback: Outcome,
) -> Result<Outcome, Failure> {
  let live = inspect_agent(herdr, name)?.ok_or_else(|| {
    Failure::refusal("agent disappeared before prompt acknowledgement was verified")
  })?;
  require_identity(name, &live, expected)?;
  if live.status != "blocked" {
    return Ok(fallback);
  }
  let read = herdr
    .call(&Call::Read {
      target: name.to_owned(),
    })
    .map_err(|err| Failure {
      message: format!(
        "cannot inspect blocked prompt acknowledgement: {}",
        err.message
      ),
      uncertain: true,
      ..err
    })?;
  let tail = read
    .pointer("/read/text")
    .and_then(Value::as_str)
    .unwrap_or("");
  Ok(acknowledged_status(&live.status, tail, fallback))
}

fn send(herdr: &mut Herdr<'_>, name: &str, keys: &[&str]) -> Result<(), Failure> {
  let keys = keys.iter().map(|key| (*key).to_owned()).collect();
  herdr
    .call(&Call::SendKeys {
      target: name.to_owned(),
      keys,
    })
    .map(|_| ())
}

fn prompt(herdr: &mut Herdr<'_>, name: &str, text: &str) -> Result<(), Failure> {
  herdr
    .call(&Call::Prompt {
      target: name.to_owned(),
      text: text.to_owned(),
    })
    .map(|_| ())
}

fn settle(herdr: &mut Herdr<'_>, name: &str) -> Result<(), Failure> {
  let until = vec!["idle".to_owned(), "done".to_owned()];
  match herdr.call(&Call::Wait {
    target: name.to_owned(),
    until,
    timeout_ms: 10_000,
  }) {
    Err(err) if missing(&err) => Ok(()),
    result => result.map(|_| ()),
  }
}

/// Whether herdr reports the agent is already gone.
fn missing(err: &Failure) -> bool {
  err.code.as_deref() == Some("agent_not_found")
}

/// Ask agent `name` on host `kind` to stop its turn and exit. An agent that
/// leaves between the listing and the request is already exited, not a
/// failure (#633).
fn request_exit(herdr: &mut Herdr<'_>, name: &str, kind: HostKind) -> Result<(), Failure> {
  match send(herdr, name, &["esc"]) {
    Err(err) if missing(&err) => return Ok(()),
    result => result?,
  }
  // Codex's turn cancellation leaves tool jobs alive; /stop is separate.
  if kind == HostKind::Codex {
    settle(herdr, name)?;
    match prompt(herdr, name, "/stop") {
      Err(err) if missing(&err) => return Ok(()),
      result => result?,
    }
    settle(herdr, name)?;
  }
  match prompt(herdr, name, "/exit") {
    Err(err) if missing(&err) => return Ok(()),
    result => result?,
  }
  if kind != HostKind::Muse {
    return Ok(());
  }
  // Live background work makes Muse ask "Exit anyway?"; enter confirms it. An
  // idle Muse has already left: the key lands at the pane shell, or herdr no
  // longer knows the agent, and the exit wait decides.
  match send(herdr, name, &["enter"]) {
    Err(err) if err.code.as_deref() != Some("agent_not_found") => Err(err),
    Ok(()) | Err(_) => Ok(()),
  }
}

/// How long an agent may take to leave herdr's list after `/exit`
/// (`EPIC_AGENT_EXIT_WAIT_S`, 30).
pub(crate) fn exit_wait(env: &Env) -> Duration {
  Duration::from_secs(number(env, "EPIC_AGENT_EXIT_WAIT_S", 30))
}

/// How long its worktree processes may take to follow it
/// (`EPIC_AGENT_EXIT_GRACE_S`, 10).
pub(crate) fn exit_grace(env: &Env) -> Duration {
  Duration::from_secs(number(env, "EPIC_AGENT_EXIT_GRACE_S", 10))
}

/// The pane's shell pid in a `pane.process_info` reply.
///
/// # Errors
/// The reply names no shell pid.
fn shell_pid(info: &Value) -> Result<u32, Failure> {
  info
    .pointer("/process_info/shell_pid")
    .and_then(Value::as_u64)
    .and_then(|pid| u32::try_from(pid).ok())
    .ok_or_else(|| Failure::refusal("pane shell ownership unavailable"))
}

/// Ask the agent to exit, wait for herdr to stop listing it
/// ([`exit_wait`]), then for its workload to go ([`exit_grace`]).
///
/// # Errors
/// The identity differs, the pane's shell is unknown, or herdr fails.
pub(crate) fn shutdown_agent(
  herdr: &mut Herdr<'_>,
  name: &str,
  expected: &Expected<'_>,
) -> Result<Outcome, Failure> {
  let live = inspect_agent(herdr, name)?;
  if let Some(live) = &live {
    require_identity(name, live, expected)?;
  }
  // A pane herdr no longer knows while the agent is unlisted too proves the
  // shutdown: there is no shell to verify (#633). A listed agent with a gone
  // pane is inconsistent and still fails.
  let shell = match herdr.call(&Call::PaneInfo {
    pane_id: expected.pane.to_owned(),
  }) {
    Err(err) if err.code.as_deref() == Some("pane_not_found") && live.is_none() => None,
    result => Some(shell_pid(&result?)?),
  };
  let cwd = Path::new(expected.cwd);
  if shell.is_none() && !cwd.exists() {
    return Ok(Outcome::Exited);
  }
  let owned = owned_workload(cwd, shell).map_err(Failure::unknown)?;
  if live.is_some() {
    request_exit(herdr, name, expected.kind)?;
    let deadline = Instant::now() + exit_wait(herdr.env);
    while !listing_gone(herdr, name)? {
      if Instant::now() >= deadline {
        return Ok(Outcome::CleanupIncomplete);
      }
      std::thread::sleep(Duration::from_secs(1));
    }
  }
  let gone =
    await_owned_exit(cwd, shell, &owned, exit_grace(herdr.env)).map_err(Failure::unknown)?;
  Ok(if gone {
    Outcome::Exited
  } else {
    Outcome::CleanupIncomplete
  })
}

#[cfg(test)]
#[path = "tests/lifecycle_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/lifecycle_shutdown_test.rs"]
mod shutdown_tests;
