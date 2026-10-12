//! Per-epic engine state: the epic's graph and reference, its own
//! `max_parallel`, and the launch batch it is working through. Epic-level
//! journal records carry the epic key in `key`.

use std::path::Path;

use serde_json::{Value, json};

use crate::graph::Graph;
use crate::model::{Pending, World};

/// One member of a launch batch and how its launch ended (`applied`,
/// `wait`, `hold`, `skipped` or `failed`), if it has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Member {
  /// The issue key.
  pub key: String,
  /// The launch outcome.
  pub outcome: Option<String>,
}

/// One registered epic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Epic {
  /// The epic key.
  pub key: String,
  /// The state directory.
  pub state_dir: String,
  /// The tracker reference.
  pub reference: String,
  /// Live issue agents at once.
  pub max_parallel: u32,
  /// The full graph, when `graph.json` is one.
  pub graph: Option<Graph>,
  /// The batch being launched, in pick order.
  pub batch: Vec<Member>,
  /// `github`, `jira` or `linear`.
  pub tracker: Option<String>,
  /// The code repository of Jira and Linear children.
  pub default_repo: Option<String>,
  /// Every sub-issue is done.
  pub complete: bool,
  /// The epic was closed in its tracker.
  pub closed: bool,
  /// A close failed or is uncertain: only a human closes the epic now.
  pub close_blocked: bool,
  /// The epic-level action in progress (refresh or close).
  pub pending: Option<Pending>,
  /// Refreshes that failed in a row.
  pub refresh_failures: u32,
  /// When the next refresh is due.
  pub refresh_at_ms: u64,
}

/// Jira and Linear epics have no change feed; they refresh every 15 minutes.
const POLL_MS: u64 = 15 * 60 * 1000;

impl Epic {
  /// A fresh epic with no batch; a Jira or Linear one polls.
  pub(crate) fn new(key: &str, state_dir: &str, now_ms: u64, tracker: Option<&str>) -> Self {
    let polled = matches!(tracker, Some("jira" | "linear"));
    Self {
      key: key.to_owned(),
      state_dir: state_dir.to_owned(),
      reference: String::new(),
      max_parallel: 1,
      graph: None,
      batch: Vec::new(),
      tracker: tracker.map(str::to_owned),
      default_repo: None,
      complete: false,
      closed: false,
      close_blocked: false,
      pending: None,
      refresh_failures: 0,
      refresh_at_ms: if polled {
        now_ms.saturating_add(POLL_MS)
      } else {
        u64::MAX
      },
    }
  }

  /// The next poll of a Jira or Linear epic, from `now_ms`.
  pub(crate) fn next_poll(&self, now_ms: u64) -> u64 {
    if matches!(self.tracker.as_deref(), Some("jira" | "linear")) {
      now_ms.saturating_add(POLL_MS)
    } else {
      u64::MAX
    }
  }
}

/// Record `key`'s epic from its `graph.json` document, keeping its batch,
/// pending action and refresh state.
pub(crate) fn load_epic_state(world: &mut World, key: &str, state_dir: &str, graph: &Value) {
  let max_parallel = graph
    .get("max_parallel")
    .and_then(Value::as_u64)
    .and_then(|limit| u32::try_from(limit).ok())
    .unwrap_or(world.max_parallel)
    .max(1);
  let tracker = graph
    .get("tracker")
    .and_then(Value::as_str)
    .map(str::to_owned);
  let mut epic = world
    .epics
    .remove(key)
    .unwrap_or_else(|| Epic::new(key, state_dir, world.now_ms, tracker.as_deref()));
  graph
    .pointer("/epic/ref")
    .and_then(Value::as_str)
    .unwrap_or("")
    .clone_into(&mut epic.reference);
  epic.max_parallel = max_parallel;
  epic.default_repo = graph
    .get("default_repo")
    .and_then(Value::as_str)
    .map(str::to_owned);
  epic.complete = graph.get("complete") == Some(&Value::Bool(true));
  epic.closed =
    epic.closed || graph.pointer("/epic/state").and_then(Value::as_str) == Some("closed");
  epic.tracker = tracker;
  epic.graph = serde_json::from_value::<Graph>(graph.clone()).ok();
  world.epics.insert(key.to_owned(), epic);
}

/// The open blockers of a graph issue as issue keys: a reference the graph
/// knows maps to its key; anything else stays as written.
pub(crate) fn blocker_keys(graph: &Value, blockers: &[String]) -> Vec<String> {
  let issues = graph
    .get("issues")
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default();
  blockers
    .iter()
    .map(|blocker| {
      issues
        .iter()
        .find(|issue| issue.get("ref").and_then(Value::as_str) == Some(blocker.as_str()))
        .and_then(|issue| issue.get("key").and_then(Value::as_str))
        .unwrap_or(blocker)
        .to_owned()
    })
    .collect()
}

/// The launch limit of `epic`.
pub(crate) fn max_parallel(world: &World, epic: &str) -> u32 {
  world
    .epics
    .get(epic)
    .map_or(world.max_parallel, |epic| epic.max_parallel)
}

/// How the engine launches an epic's workers, as `toolu epic start` recorded
/// it in `<state_dir>/launch.json`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LaunchSettings {
  /// Keep host approval prompts on (`--safe`).
  pub safe: bool,
  /// Route and check coupling by heuristics only, sending nothing to Jev
  /// (`--no-jev`).
  pub no_jev: bool,
}

fn settings_path(state_dir: &Path) -> std::path::PathBuf {
  state_dir.join("launch.json")
}

/// Record `settings` for the engine's launches; each start replaces them.
///
/// # Errors
/// The settings file cannot be written.
pub(crate) fn write_launch_settings(
  state_dir: &Path,
  settings: LaunchSettings,
) -> Result<(), String> {
  let doc = json!({"safe": settings.safe, "no_jev": settings.no_jev});
  crate::disk::write_value(&settings_path(state_dir), &doc)
}

/// The epic's launch settings. Without a file both are off; a file that
/// cannot be read or is blank, or a setting that is not `true` or `false`,
/// turns that setting on, so a damaged file never drops prompts or sends text
/// to Jev.
pub(crate) fn launch_settings(state_dir: &Path) -> LaunchSettings {
  let path = settings_path(state_dir);
  // Only a file that is certainly absent means the defaults; a state dir that
  // cannot be inspected reads as damaged.
  if matches!(path.try_exists(), Ok(false)) {
    return LaunchSettings::default();
  }
  let doc = crate::disk::read_value(&path).unwrap_or(Value::Null);
  let flag = |name: &str| match doc.get(name) {
    Some(Value::Bool(on)) => *on,
    None if doc.is_object() => false,
    _ => true,
  };
  LaunchSettings {
    safe: flag("safe"),
    no_jev: flag("no_jev"),
  }
}

#[cfg(test)]
#[path = "tests/epics_test.rs"]
mod tests;
