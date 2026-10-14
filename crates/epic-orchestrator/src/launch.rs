//! Launch or resume one sub-issue (`launch-issue.ts`): a herdr worktree, the
//! routed host's agent and the worker brief, under a durable lease reserved
//! before any external mutation. A dry run prints the commands and the brief
//! and writes nothing.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde_json::{Map, Value, json};
use toolu_runtime::env::Env;

pub(crate) mod admission;
mod agent;
mod attempt;
mod brief;
mod checkout;
mod deliver;
mod digest;
mod failure;
pub(crate) mod lifecycle;
pub(crate) mod record;
mod session;
mod worktree;

pub(crate) use agent::Resolved;
pub(crate) use checkout::guard_budget;
pub(crate) use failure::Failure;
pub(crate) use lifecycle::Herdr;

use crate::graph::{Graph, GraphIssue};
use crate::hosts::host_kind;
use crate::route::{LaunchRoute, load_launch_route};
use crate::tracker::Gh;

/// The flags of one launch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LaunchOpts {
  /// An explicit host; otherwise the route's, otherwise Claude.
  pub kind: Option<String>,
  /// An explicit model.
  pub model: Option<String>,
  /// An explicit effort.
  pub effort: Option<String>,
  /// Claude's permission mode when attended.
  pub permission_mode: String,
  /// Keep approval prompts on.
  pub safe: bool,
  /// Print the commands and the brief; write nothing.
  pub dry_run: bool,
  /// Launch a blocked issue, or skip the budget guard.
  pub force: bool,
  /// How an earlier attempt is taken over.
  pub recovery: Recovery,
}

/// How a launch takes over an earlier attempt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Recovery {
  /// Refuse an uncertain attempt.
  #[default]
  Fresh,
  /// Prompt again after an uncertain attempt (`--reprompt`).
  Reprompt,
  /// Exit a live agent first, e.g. to move the issue to another host (`--replace`).
  Replace,
}

impl LaunchOpts {
  /// `--replace`.
  pub(crate) fn replace(&self) -> bool {
    self.recovery == Recovery::Replace
  }

  /// `--reprompt`, which `--replace` includes.
  pub(crate) fn reprompt(&self) -> bool {
    self.recovery != Recovery::Fresh
  }
}

/// What a launch reaches outside the process.
pub(crate) struct Ctx<'a> {
  /// The environment.
  pub env: &'a Env,
  /// herdr.
  pub herdr: Herdr<'a>,
  /// GitHub, or why it is unavailable.
  pub gh: Result<&'a Gh, String>,
  /// How long to wait for the host to record its session.
  pub session_wait: Duration,
}

impl<'a> Ctx<'a> {
  /// The context of one engine effect or verb: live herdr through `paths`,
  /// and ten seconds for the host to record its session.
  pub(crate) fn live(
    env: &'a Env,
    paths: &'a crate::paths::Paths,
    gh: Result<&'a Gh, String>,
  ) -> Self {
    Self {
      env,
      herdr: Herdr {
        client: crate::herdr::Client::new(),
        paths,
        env,
      },
      gh,
      session_wait: Duration::from_secs(10),
    }
  }
}

/// Explicit flags win; a route supplies host, model and effort, and its model
/// and effort apply only when its host is the one launching.
///
/// # Errors
/// The route has no host, or the host is unknown.
pub(crate) fn resolve_host(
  opts: &LaunchOpts,
  route: Option<&LaunchRoute>,
) -> Result<Resolved, Failure> {
  if route.is_some_and(|route| route.host.is_none()) {
    return Err(Failure::unknown(
      "no host capacity: reroute after a slot becomes available",
    ));
  }
  let routed = route.and_then(|route| route.host);
  let kind = match (&opts.kind, routed) {
    (Some(kind), _) => host_kind(kind).map_err(Failure::unknown)?,
    (None, Some(kind)) => kind,
    (None, None) => crate::hosts::HostKind::Claude,
  };
  let from_route = route.filter(|route| route.host == Some(kind));
  Ok(Resolved {
    kind,
    model: opts
      .model
      .clone()
      .or_else(|| from_route.and_then(|route| route.model.clone())),
    effort: opts
      .effort
      .clone()
      .or_else(|| from_route.and_then(|route| route.effort.clone())),
  })
}

/// The sub-issue `wanted` names by reference, key or URL.
///
/// # Errors
/// It is not a sub-issue of the epic.
pub(crate) fn find_issue<'g>(graph: &'g Graph, wanted: &str) -> Result<&'g GraphIssue, Failure> {
  graph.find(wanted).ok_or_else(|| {
    Failure::unknown(format!(
      "issue {} is not a sub-issue of {}",
      Value::String(wanted.to_owned()),
      graph.epic.reference
    ))
  })
}

fn require_recoverable_stage(record: &record::Record, opts: &LaunchOpts) -> Result<(), Failure> {
  match record::text(record, "stage") {
    Some("cleaning" | "cleanup-incomplete") => Err(Failure::refusal(
      "cleanup is incomplete; reconcile with toolu epic finish before launching again",
    )),
    Some("replacing") if !opts.replace() => Err(Failure::refusal(
      "replacement is incomplete; retry explicitly with --replace",
    )),
    Some("starting" | "uncertain") if !opts.reprompt() => Err(Failure::refusal(
      "previous launch/prompt outcome is uncertain; inspect the recorded pane/session before explicit --reprompt or --replace",
    )),
    _ => Ok(()),
  }
}

fn epoch_ms(at: SystemTime) -> u64 {
  toolu_state::time::epoch_millis(at)
}

/// Launch `issue`, taking the per-issue launch lock unless it is a dry run.
///
/// # Errors
/// Any refusal or failure; a failure after reservation leaves the record
/// `uncertain` with its lease.
pub(crate) fn launch_issue(
  graph: &Graph,
  issue: &GraphIssue,
  opts: &LaunchOpts,
  ctx: &mut Ctx<'_>,
) -> Result<Value, Failure> {
  if opts.dry_run {
    return launch(graph, issue, opts, ctx);
  }
  let state = Path::new(&graph.state_dir).to_path_buf();
  admission::with_launch_ownership(&state, &issue.key, || launch(graph, issue, opts, ctx))
}

fn launch(
  graph: &Graph,
  issue: &GraphIssue,
  opts: &LaunchOpts,
  ctx: &mut Ctx<'_>,
) -> Result<Value, Failure> {
  if !matches!(issue.status.as_str(), "ready" | "in_flight") && !opts.force {
    return Err(Failure::unknown(format!(
      "{} is {} (open blockers: {}); pass --force to launch anyway",
      issue.reference,
      issue.status,
      Value::from(issue.open_blockers.clone())
    )));
  }
  let state = Path::new(&graph.state_dir);
  let mut record =
    record::load_launch_record(&state.join("issues").join(format!("{}.json", issue.key)))?;
  let route = load_launch_route(&state.join("routes").join(format!("{}.json", issue.key)))?;
  let host = resolve_host(opts, route.as_ref())?;
  require_recoverable_stage(&record, opts)?;
  let previous = record::text(&record, "kind").map(str::to_owned);
  if previous
    .as_deref()
    .is_some_and(|kind| kind != host.kind.name())
    && !opts.replace()
  {
    return Err(Failure::refusal("host change requires explicit --replace"));
  }
  let launches = record.get("launches").and_then(Value::as_u64).unwrap_or(0);
  if !opts.dry_run && launches == 0 && !opts.force {
    checkout::guard_budget(ctx.gh.clone(), ctx.env)?;
  }
  let mut run = attempt::Attempt::new(graph, issue, opts, host, epoch_ms(SystemTime::now()));
  run.resuming = !record.is_empty() || issue.status == "in_flight";
  run.previous = previous;
  run.launches = launches;
  run.reserve(&mut record, ctx)?;
  let outcome = run.go(&mut record, ctx);
  run.settle(&mut record, outcome)
}

/// The dry-run text: `# <ref>`, the commands, the brief path, a blank line
/// and the brief.
pub(crate) fn render_dry(result: &Value) -> String {
  let mut lines = vec![format!("# {}", result["issue"].as_str().unwrap_or(""))];
  let commands = result
    .get("commands")
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default();
  lines.extend(commands.iter().filter_map(Value::as_str).map(str::to_owned));
  lines.push(format!(
    "# brief -> {}",
    result
      .get("brief_path")
      .and_then(Value::as_str)
      .unwrap_or("")
  ));
  lines.push(String::new());
  lines.push(
    result
      .get("brief")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned(),
  );
  let mut text = lines.join("\n");
  text.push('\n');
  text
}

/// `{"issue": …, "commands": …}` followed by every record field.
pub(crate) fn live_result(issue: &str, commands: &[String], record: &Map<String, Value>) -> Value {
  let mut out = Map::new();
  out.insert("issue".to_owned(), json!(issue));
  out.insert("commands".to_owned(), json!(commands));
  out.extend(record.clone());
  Value::Object(out)
}

#[cfg(test)]
#[path = "tests/launch_test.rs"]
mod tests;
