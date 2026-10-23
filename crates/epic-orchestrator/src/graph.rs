//! An epic's sub-issue dependency graph and the next launch batch
//! (`scripts/epic-graph.ts`): classification, waves, unblocked work, chain
//! length, the batch that fits the free slots, cycles and local checkouts.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::tracker::{EpicInfo, PrNode};

mod batch;
mod checkouts;
mod classify;
mod from_tracker;
mod levels;
mod render;

pub(crate) use batch::pick_batch;
pub(crate) use checkouts::{herdr_roots, repo_roots, resolve_checkouts, resolve_clone_root};
pub(crate) use classify::{classify, occupies_slot};
pub(crate) use from_tracker::{BuildInput, build};
pub(crate) use levels::{chain_lengths, compute_levels, downstream_counts};
pub(crate) use render::render_table;

/// One sub-issue in `graph.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GraphIssue {
  /// Canonical tracker reference.
  #[serde(rename = "ref")]
  pub reference: String,
  /// Title.
  pub title: String,
  /// `open` or `closed`.
  pub state: String,
  /// Web URL.
  pub url: String,
  /// The GitHub repository of the code.
  pub repo: String,
  /// GitHub issue number, or `None`.
  pub number: Option<u64>,
  /// Blocker reference to its state.
  #[serde(default)]
  pub blockers: BTreeMap<String, String>,
  /// Closing pull requests.
  #[serde(default)]
  pub prs: Vec<PrNode>,
  /// Where the blockers came from.
  #[serde(default)]
  pub deps_source: String,
  /// Labels.
  #[serde(default)]
  pub labels: Vec<String>,
  /// Description excerpt.
  #[serde(default)]
  pub excerpt: String,
  /// The herdr agent name.
  pub key: String,
  /// `done`, `in_flight`, `ready`, `blocked` or `external_blocked`.
  #[serde(default)]
  pub status: String,
  /// Open blockers, sorted.
  #[serde(default)]
  pub open_blockers: Vec<String>,
  /// Wave, or `None` when done or in a cycle.
  #[serde(default)]
  pub wave: Option<u64>,
  /// How many issues transitively wait on this one.
  #[serde(default)]
  pub unblocks: u64,
  /// The longest dependent chain starting here.
  #[serde(default)]
  pub chain: u64,
  /// The local checkout of `repo`.
  #[serde(default)]
  pub checkout: Option<String>,
  /// The issue branch.
  #[serde(default)]
  pub branch: String,
  /// The launch record's stage.
  #[serde(default)]
  pub stage: Option<String>,
}

/// Issues per status.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Counts {
  /// Closed in the tracker and not occupied.
  pub done: u64,
  /// Occupied or with an open PR.
  pub in_flight: u64,
  /// Launchable.
  pub ready: u64,
  /// Waiting on a blocker in the epic.
  pub blocked: u64,
  /// Waiting on a blocker outside the epic.
  pub external_blocked: u64,
}

/// The whole `graph.json` document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Graph {
  /// `github`, `jira` or `linear`.
  #[serde(default)]
  pub tracker: Option<String>,
  /// The epic.
  pub epic: EpicInfo,
  /// The default code repository of Jira and Linear children.
  #[serde(default)]
  pub default_repo: Option<String>,
  /// The epic's state directory.
  pub state_dir: String,
  /// Live issue agents at once.
  #[serde(default = "default_max")]
  pub max_parallel: u64,
  /// Every sub-issue, in tracker order.
  pub issues: Vec<GraphIssue>,
  /// Issues per status.
  #[serde(default)]
  pub counts: Counts,
  /// In-flight references.
  #[serde(default)]
  pub in_flight: Vec<String>,
  /// Ready references.
  #[serde(default)]
  pub ready: Vec<String>,
  /// The ready issues that fit the free slots, most-unblocking first.
  #[serde(default)]
  pub launch_batch: Vec<String>,
  /// References in a dependency cycle; non-empty means nothing is scheduled.
  #[serde(default)]
  pub cycle: Vec<String>,
  /// Repositories with no local checkout.
  #[serde(default)]
  pub missing_checkouts: Vec<String>,
  /// Where missing checkouts are cloned.
  #[serde(default)]
  pub clone_root: String,
  /// Every sub-issue is done.
  #[serde(default)]
  pub complete: bool,
}

fn default_max() -> u64 {
  3
}

impl Graph {
  /// The issue whose reference, key or URL is `wanted`.
  pub(crate) fn find(&self, wanted: &str) -> Option<&GraphIssue> {
    self
      .issues
      .iter()
      .find(|issue| issue.reference == wanted || issue.key == wanted || issue.url == wanted)
  }
}

/// Read `graph.json`.
///
/// # Errors
/// The file cannot be read or is not a graph.
pub(crate) fn read_graph(path: &Path) -> Result<Graph, String> {
  let text = std::fs::read_to_string(path)
    .map_err(|err| format!("cannot read graph {}: {err}", path.display()))?;
  serde_json::from_str(&text).map_err(|err| format!("cannot read graph {}: {err}", path.display()))
}

/// Write `graph` as pretty JSON with a trailing newline.
///
/// # Errors
/// The directory or file cannot be written.
pub(crate) fn write_graph(path: &Path, graph: &Graph) -> Result<(), String> {
  crate::disk::write_pretty(path, graph)
}

#[cfg(test)]
#[path = "tests/graph_test.rs"]
mod tests;
