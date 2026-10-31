//! The engine's in-process launch effect: the coupling check against the
//! batch members ahead, then routing (a valid saved route is kept), then the
//! launch itself. The outcome is a JSON document: `applied`, `hold` (with
//! the partner, score and source), `wait` (no host, shared capacity, a
//! resource hold, the GitHub budget, or a pane whose shell never became
//! ready), `settled` (a finish already merged or abandoned the issue, so it
//! is never relaunched, #619), `needs-human` (the agent waits on an approval
//! screen, #626) or `failed` (both with the record's stage after the attempt).

use std::path::PathBuf;
use std::time::SystemTime;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::coupling::check;
use crate::executor::Job;
use crate::graph::{Graph, GraphIssue, read_graph};
use crate::hosts::HostKind;
use crate::launch::{
  Ctx, FINISHED, Failure, LaunchOpts, Launcher, PANE_BUSY, Recovery, finished_stage, guard_budget,
  launch_issue,
};
use crate::paths::Paths;
use crate::retry::{Policy, effect_scope};
use crate::route::{RouteOptions, jev_from_env, load_host_pool, load_table, route_issues};

/// What one launch effect needs, owned so it can run on the effect thread.
pub(crate) struct LaunchJob {
  /// The epic state directory.
  pub state_dir: PathBuf,
  /// The issue key.
  pub key: String,
  /// The batch members ahead it must not overlap.
  pub ahead: Vec<String>,
  /// Route again and replace the agent (after a host limit).
  pub reroute: bool,
  /// The environment.
  pub env: Env,
  /// The engine paths, for herdr's CLI journal.
  pub paths: Paths,
  /// The client builders.
  pub reach: crate::reach::Reach,
}

/// The launch effect for `input`. Its retries share one sleep budget.
pub(crate) fn job(input: LaunchJob) -> Job {
  Box::new(move || {
    let policy = Policy::from_env(&input.env);
    effect_scope(&policy, || run(&input)).to_string()
  })
}

fn failed(note: &str, stage: Option<&str>) -> Value {
  json!({"outcome": "failed", "note": note, "stage": stage})
}

fn settled(stage: Option<&str>) -> Value {
  json!({"outcome": "settled", "stage": stage})
}

/// Whether a launch refusal only waits for capacity, for a resource hold to
/// clear, or for a new pane's shell that never became ready (nothing started).
fn waits(message: &str) -> bool {
  message.contains("capacity exhausted")
    || message.contains("no host capacity")
    || message.contains("resource hold:")
    || message.contains(PANE_BUSY)
}

/// Whether this launch may ask Jev: a key is set and the epic was not started
/// with `--no-jev`, which keeps routing and the coupling check heuristic so no
/// issue text reaches Jev.
fn jev_allowed(input: &LaunchJob) -> bool {
  let keyed = input
    .env
    .get("TYPESAFE_API_KEY")
    .is_some_and(|key| !key.is_empty());
  keyed && !crate::epics::launch_settings(&input.state_dir).no_jev
}

/// The issue's launch record, when it has one.
fn record(input: &LaunchJob) -> Option<Value> {
  let path = input
    .state_dir
    .join("issues")
    .join(format!("{}.json", input.key));
  crate::disk::read_value(&path).ok()
}

/// One string field of the issue's launch record.
fn record_field(input: &LaunchJob, name: &str) -> Option<String> {
  record(input)?.get(name)?.as_str().map(str::to_owned)
}

fn run(input: &LaunchJob) -> Value {
  // A settled issue may have left the graph: settle before reading it.
  if let Some(Value::Object(record)) = record(input)
    && let Some(stage) = finished_stage(&record)
  {
    return settled(Some(stage));
  }
  let graph = match read_graph(&input.state_dir.join("graph.json")) {
    Ok(graph) => graph,
    Err(err) => return failed(&err, None),
  };
  let Some(issue) = graph.find(&input.key) else {
    return failed(&format!("{} is not in the graph", input.key), None);
  };
  // A launch armed before a refresh showed the tracker item done, or one
  // recovered from the journal, never passed refill's check again.
  if issue.status == "done" && record_field(input, "stage").is_none() {
    return settled(None);
  }
  let jev = jev_from_env(&input.env);
  let keyed = jev_allowed(input);
  let jev_arg = || keyed.then(|| jev.as_ref().map_err(Clone::clone));
  let ahead: Vec<&GraphIssue> = input
    .ahead
    .iter()
    .filter_map(|key| graph.find(key))
    .collect();
  let verdict = check(issue, &ahead, jev_arg());
  if let Some(hold) = verdict.hold {
    return json!({"outcome": "hold", "with": hold.with, "score": hold.score, "source": hold.source, "fallback": verdict.fallback});
  }
  let mut result = launch_routed(input, &graph, issue, jev_arg());
  if let (Some(map), Some(reason)) = (result.as_object_mut(), verdict.fallback) {
    map.insert("fallback".to_owned(), json!(reason));
  }
  result
}

fn launch_routed(
  input: &LaunchJob,
  graph: &Graph,
  issue: &GraphIssue,
  jev: Option<Result<&toolu_jev_client::Jev, String>>,
) -> Value {
  // A relaunch of a running worker whose host is paused waits for the
  // resume, keeping its saved route and session; it never reroutes.
  if paused_relaunch(input) {
    return json!({"outcome": "wait", "note": "host paused"});
  }
  let gh = (input.reach.gh)(&input.env);
  // `force` below only passes a stale graph status: a first launch still
  // waits while the GitHub budget is under its floor.
  let launches = record(input).and_then(|record| record.get("launches")?.as_u64());
  if launches.unwrap_or(0) == 0
    && let Err(err) = guard_budget(gh.as_ref().map_err(Clone::clone), &input.env)
  {
    return json!({"outcome": "wait", "note": err.message});
  }
  let route = match route_for(input, graph, issue, jev) {
    Ok(Some(route)) => route,
    Ok(None) => return json!({"outcome": "wait", "note": "no host capacity"}),
    Err(err) => return failed(&err, None),
  };
  let opts = LaunchOpts {
    kind: route.host,
    model: route.model,
    effort: route.effort,
    permission_mode: "auto".to_owned(),
    safe: crate::epics::launch_settings(&input.state_dir).safe,
    dry_run: false,
    force: true,
    // A reroute, or a relaunch after the agent was lost, replaces the
    // recorded agent in the same worktree with the resume prompt.
    recovery: if input.reroute || record_field(input, "stage").as_deref() == Some("running") {
      Recovery::Replace
    } else {
      Recovery::Fresh
    },
    by: Launcher::Engine,
  };
  let mut ctx = Ctx::live(&input.env, &input.paths, gh.as_ref().map_err(Clone::clone));
  let result = launch_issue(graph, issue, &opts, &mut ctx).map(|_document| ());
  outcome_of(result, |name| record_field(input, name))
}

/// Whether this launch is a non-reroute relaunch of a running worker whose
/// recorded host is paused in the shared store.
fn paused_relaunch(input: &LaunchJob) -> bool {
  if input.reroute || record_field(input, "stage").as_deref() != Some("running") {
    return false;
  }
  let Some(kind) =
    record_field(input, "kind").and_then(|name| crate::hosts::parse_host_kind(&name))
  else {
    return false;
  };
  let root = toolu_engine::resources::store::resource_home(&input.env);
  crate::host_pause::is_paused(&root, kind).unwrap_or(false)
}

/// The effect's outcome for a launch that `launched` or failed, reading the
/// record's fields after the attempt through `field`.
fn outcome_of(launched: Result<(), Failure>, field: impl Fn(&str) -> Option<String>) -> Value {
  match launched {
    Ok(()) => json!({"outcome": "applied", "kind": field("kind"),
      "agent":field("agent"),"pane_id":field("pane_id"),"worktree":field("worktree")}),
    Err(err) if waits(&err.message) => json!({"outcome": "wait", "note": err.message}),
    Err(err) if err.message.starts_with(FINISHED) => settled(field("stage").as_deref()),
    Err(err) if err.awaits_approval() => json!({
      "outcome": "needs-human",
      "note": err.message,
      "stage": field("stage"),
    }),
    Err(err) => failed(&err.message, field("stage").as_deref()),
  }
}

/// The issue's route: the saved one while valid, else a new one; `None`
/// when no host has capacity.
fn route_for(
  input: &LaunchJob,
  graph: &Graph,
  issue: &GraphIssue,
  jev: Option<Result<&toolu_jev_client::Jev, String>>,
) -> Result<Option<crate::route::Route>, String> {
  let fallback_pool = vec![(HostKind::Claude, graph.max_parallel)];
  let pool = load_host_pool(&input.state_dir.join("pool.json"), fallback_pool)?;
  let table = load_table(&input.env);
  let options = RouteOptions {
    jev,
    reroute: input.reroute,
    table: &table,
    resource_root: Some(toolu_engine::resources::store::resource_home(&input.env)),
    now_ms: toolu_state::time::epoch_millis(SystemTime::now()),
    persist: true,
  };
  let (routes, _) = route_issues(graph, std::slice::from_ref(&issue.key), &pool, &options)?;
  Ok(
    routes
      .into_iter()
      .next()
      .filter(|route| route.host.is_some()),
  )
}

#[cfg(test)]
#[path = "tests/effects_launch_test.rs"]
mod tests;
