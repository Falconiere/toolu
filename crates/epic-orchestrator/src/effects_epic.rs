//! The epic-level effects: refresh rebuilds `graph.json` from the tracker,
//! close posts the summary and closes the epic once, and gc sweeps the closed
//! epic's machine resources (`toolu_engine::gc`). A recovered close uses
//! the durable comment intent to resume without a second comment.

use std::path::PathBuf;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::close::close_epic;
use crate::executor::Job;
use crate::graph::{BuildInput, build, herdr_roots, read_graph, repo_roots, write_graph};
use crate::model::Action;
use crate::paths::Paths;
use crate::reach::{Reach, redact};
use crate::retry::{Policy, effect_scope};
use crate::tracker::{TrackerKind, detect_tracker};

/// What one epic effect needs, moved onto the effect thread.
pub(crate) struct EpicJob {
  /// The epic state directory.
  pub state_dir: PathBuf,
  /// The epic reference.
  pub reference: String,
  /// `github`, `jira` or `linear`, when the graph names it.
  pub tracker: Option<String>,
  /// The code repository of Jira and Linear children.
  pub default_repo: String,
  /// Live issue agents at once.
  pub max_parallel: u64,
  /// The environment effects run with.
  pub env: Env,
  /// The engine's resource root, for herdr.
  pub paths: Paths,
  /// The client builders.
  pub reach: Reach,
}

/// The body of `action` (refresh or close) for `input`.
pub(crate) fn job(action: Action, input: EpicJob) -> Job {
  Box::new(move || {
    let policy = Policy::from_env(&input.env);
    effect_scope(&policy, || run(action, &input)).to_string()
  })
}

/// Run `action` and describe its outcome.
pub(crate) fn run(action: Action, input: &EpicJob) -> Value {
  let result = match action {
    Action::Close => close(input),
    Action::Refresh => refresh(input),
    Action::Gc => Ok(gc(input)),
    Action::Merge
    | Action::Cleanup
    | Action::Launch
    | Action::Checkpoint
    | Action::Reconcile
    | Action::Prompt => Err(format!("{} is not an epic action", action.name())),
  };
  result.unwrap_or_else(|err| json!({"outcome": "failed", "note": redact(&input.env, &err)}))
}

fn refresh(input: &EpicJob) -> Result<Value, String> {
  let kind = detect_tracker(&input.reference, input.tracker.as_deref(), &input.env)?;
  let tracker = (input.reach.tracker)(kind, &input.reference, &input.default_repo, &input.env)?;
  let gh = if kind == TrackerKind::Github {
    None
  } else {
    (input.reach.gh)(&input.env).ok()
  };
  let built = build(&BuildInput {
    tracker: tracker.as_ref(),
    default_repo: input.default_repo.clone(),
    state_dir: input.state_dir.clone(),
    max_parallel: input.max_parallel,
    gh: gh.as_ref(),
    herdr_roots: herdr_roots(&input.paths, &input.env),
    repo_roots: repo_roots(&input.env),
  })?;
  write_graph(&input.state_dir.join("graph.json"), &built)?;
  Ok(json!({"outcome": "applied", "complete": built.complete}))
}

fn close(input: &EpicJob) -> Result<Value, String> {
  let graph = read_graph(&input.state_dir.join("graph.json"))?;
  let kind = detect_tracker(&graph.epic.reference, graph.tracker.as_deref(), &input.env)?;
  let repo = graph.default_repo.clone().unwrap_or_default();
  let tracker = (input.reach.tracker)(kind, &graph.epic.reference, &repo, &input.env)?;
  let now = toolu_state::time::epoch_millis(std::time::SystemTime::now());
  let metrics = crate::metrics::closing(&input.paths, &input.state_dir, now);
  let result = close_epic(&graph, tracker.as_ref(), metrics.as_deref())?;
  Ok(json!({"outcome": "applied", "result": result}))
}

/// Sweep the epic's checkouts (its records' and the configured roots), its
/// state directory and the resource root; the VMs through this binary.
fn gc(input: &EpicJob) -> Value {
  let mut repos = repo_roots(&input.env);
  repos.extend(herdr_roots(&input.paths, &input.env));
  // shortcut: the sandbox and VM steps run only when the engine is the
  // `toolu` binary itself, so a test binary hosting the engine never touches
  // a developer's VM; upgrade if the engine ever ships under another name.
  let toolu = toolu_runtime::invocation::current_exe()
    .filter(|exe| exe.file_name().is_some_and(|name| name == "toolu"));
  let request = toolu_engine::gc::Request {
    repos,
    state_dirs: vec![input.state_dir.clone()],
    resource_root: input.paths.root.clone(),
    toolu,
    apply: true,
    all_branches: false,
    env: input.env.clone(),
  };
  json!({"outcome": "applied", "swept": toolu_engine::gc::sweep(&request)})
}

#[cfg(test)]
#[path = "tests/effects_epic_test.rs"]
mod tests;
