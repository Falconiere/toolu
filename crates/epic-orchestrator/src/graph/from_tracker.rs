//! Build `graph.json` from the tracker, the launch records and the checkouts.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{
  Counts, Graph, GraphIssue, chain_lengths, classify, compute_levels, downstream_counts,
  pick_batch, resolve_checkouts, resolve_clone_root,
};
use crate::refs::{branch_for, key_for};
use crate::tracker::{Gh, PrNode, TrackedIssue, Tracker, TrackerKind};

/// What one graph build needs.
pub(crate) struct BuildInput<'a> {
  /// The epic's tracker.
  pub tracker: &'a dyn Tracker,
  /// The default code repository of Jira and Linear children.
  pub default_repo: String,
  /// The epic's state directory.
  pub state_dir: PathBuf,
  /// Live issue agents at once.
  pub max_parallel: u64,
  /// GitHub, for the PRs of Jira and Linear children found by branch.
  pub gh: Option<&'a Gh>,
  /// herdr's primary repository roots.
  pub herdr_roots: Vec<PathBuf>,
  /// `EPIC_REPO_ROOTS`.
  pub repo_roots: Vec<PathBuf>,
}

/// The graph of the epic in `input`.
///
/// # Errors
/// A tracker call fails, or a launch record cannot be read.
pub(crate) fn build(input: &BuildInput<'_>) -> Result<Graph, String> {
  let epic = input.tracker.epic()?;
  let children = input.tracker.children()?;
  let launched = launch_records(&input.state_dir)?;
  let mut issues: Vec<GraphIssue> = children.into_iter().map(graph_issue).collect();
  if input.tracker.kind() != TrackerKind::Github
    && let Some(gh) = input.gh
  {
    attach_branch_prs(gh, &mut issues, &launched)?;
  }
  let epic_repo = match input.tracker.kind() {
    TrackerKind::Github => epic
      .reference
      .rsplit_once('#')
      .map_or_else(String::new, |(repo, _)| repo.to_owned()),
    TrackerKind::Jira | TrackerKind::Linear => input.default_repo.clone(),
  };
  let repos: Vec<String> = issues
    .iter()
    .map(|issue| issue.repo.clone())
    .chain(std::iter::once(epic_repo.clone()))
    .filter(|repo| !repo.is_empty())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect();
  let checkouts = resolve_checkouts(&repos, &input.herdr_roots, &input.repo_roots);
  annotate(&mut issues, &launched, &checkouts);
  let clone_root = resolve_clone_root(
    checkouts.get(&epic_repo).and_then(Option::as_deref),
    &input.repo_roots,
    &input.herdr_roots,
    &checkouts,
  )?;
  Ok(assemble(input, epic, issues, &checkouts, &clone_root))
}

fn graph_issue(tracked: TrackedIssue) -> GraphIssue {
  GraphIssue {
    key: key_for(&tracked.reference, &tracked.repo, tracked.number),
    reference: tracked.reference,
    title: tracked.title,
    state: tracked.state,
    url: tracked.url,
    repo: tracked.repo,
    number: tracked.number,
    blockers: tracked.blockers,
    prs: tracked.prs,
    deps_source: tracked.deps_source,
    labels: tracked.labels,
    excerpt: tracked.excerpt,
    status: String::new(),
    open_blockers: Vec::new(),
    wave: None,
    unblocks: 0,
    chain: 0,
    checkout: None,
    branch: String::new(),
    stage: None,
  }
}

/// `issues/<key>.json` of every launched issue.
fn launch_records(state_dir: &Path) -> Result<BTreeMap<String, Value>, String> {
  let dir = state_dir.join("issues");
  let entries = match std::fs::read_dir(&dir) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
    Err(err) => return Err(format!("{}: {err}", dir.display())),
  };
  let mut out = BTreeMap::new();
  for entry in entries {
    let path = entry.map_err(|err| err.to_string())?.path();
    let Some(stem) = path
      .file_name()
      .and_then(|name| name.to_str())
      .and_then(|name| name.strip_suffix(".json"))
    else {
      continue;
    };
    let record = crate::disk::read_value(&path).unwrap_or(Value::Null);
    out.insert(stem.to_owned(), record);
  }
  Ok(out)
}

fn record_text<'a>(
  launched: &'a BTreeMap<String, Value>,
  key: &str,
  field: &str,
) -> Option<&'a str> {
  launched.get(key)?.get(field)?.as_str()
}

/// Jira and Linear items have no closing-PR references: find PRs by branch.
fn attach_branch_prs(
  gh: &Gh,
  issues: &mut [GraphIssue],
  launched: &BTreeMap<String, Value>,
) -> Result<(), String> {
  for issue in issues.iter_mut() {
    let Some(branch) = record_text(launched, &issue.key, "branch") else {
      continue;
    };
    let owner = issue.repo.split_once('/').map_or("", |(owner, _)| owner);
    let path = format!(
      "/repos/{}/pulls?head={owner}:{branch}&state=all",
      issue.repo
    );
    let rows: Vec<Value> = gh.get(&path)?;
    issue.prs = rows.iter().map(pr_node).collect();
  }
  Ok(())
}

fn pr_node(row: &Value) -> PrNode {
  let merged = row.get("merged_at").is_some_and(|at| !at.is_null());
  let state = if row.get("state").and_then(Value::as_str) == Some("open") {
    "OPEN"
  } else if merged {
    "MERGED"
  } else {
    "CLOSED"
  };
  PrNode {
    number: row.get("number").and_then(Value::as_u64).unwrap_or(0),
    state: state.to_owned(),
    url: row
      .get("html_url")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned(),
    head_ref_name: row
      .pointer("/head/ref")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned(),
  }
}

fn annotate(
  issues: &mut [GraphIssue],
  launched: &BTreeMap<String, Value>,
  checkouts: &BTreeMap<String, Option<String>>,
) {
  let (levels, _) = compute_levels(issues);
  let downstream = downstream_counts(issues);
  let chains = chain_lengths(issues);
  let in_epic: BTreeSet<String> = issues.iter().map(|issue| issue.reference.clone()).collect();
  let in_epic: BTreeSet<&str> = in_epic.iter().map(String::as_str).collect();
  for issue in issues.iter_mut() {
    let stage = record_text(launched, &issue.key, "stage").map(str::to_owned);
    issue.status = classify(issue, &in_epic, stage.as_deref()).to_owned();
    issue.open_blockers = issue
      .blockers
      .iter()
      .filter(|(_, state)| *state == "open")
      .map(|(blocker, _)| blocker.clone())
      .collect();
    issue.wave = levels.get(&issue.reference).copied();
    issue.unblocks = downstream.get(&issue.reference).copied().unwrap_or(0);
    issue.chain = if issue.state == "open" {
      chains.get(&issue.reference).copied().unwrap_or(0)
    } else {
      0
    };
    issue.checkout = checkouts.get(&issue.repo).cloned().flatten();
    issue.branch = record_text(launched, &issue.key, "branch").map_or_else(
      || branch_for(&issue.reference, issue.number, &issue.title),
      str::to_owned,
    );
    issue.stage = stage;
  }
}

fn assemble(
  input: &BuildInput<'_>,
  epic: crate::tracker::EpicInfo,
  issues: Vec<GraphIssue>,
  checkouts: &BTreeMap<String, Option<String>>,
  clone_root: &Path,
) -> Graph {
  let (_, cycle) = compute_levels(&issues);
  let with = |status: &str| -> Vec<String> {
    issues
      .iter()
      .filter(|issue| issue.status == status)
      .map(|issue| issue.reference.clone())
      .collect()
  };
  let in_flight = with("in_flight");
  let ready = with("ready");
  let slots = input
    .max_parallel
    .saturating_sub(u64::try_from(in_flight.len()).unwrap_or(u64::MAX));
  Graph {
    tracker: Some(input.tracker.kind().name().to_owned()),
    epic,
    default_repo: Some(input.default_repo.clone()).filter(|repo| !repo.is_empty()),
    state_dir: input.state_dir.display().to_string(),
    max_parallel: input.max_parallel,
    counts: counts(&issues),
    launch_batch: pick_batch(
      &ready,
      &issues,
      &in_flight,
      usize::try_from(slots).unwrap_or(0),
    ),
    complete: !issues.is_empty() && issues.iter().all(|issue| issue.status == "done"),
    issues,
    in_flight,
    ready,
    cycle,
    missing_checkouts: missing(checkouts),
    clone_root: clone_root.display().to_string(),
  }
}

/// Issues per status.
fn counts(issues: &[GraphIssue]) -> Counts {
  let count = |status: &str| {
    let n = issues.iter().filter(|issue| issue.status == status).count();
    u64::try_from(n).unwrap_or(u64::MAX)
  };
  Counts {
    done: count("done"),
    in_flight: count("in_flight"),
    ready: count("ready"),
    blocked: count("blocked"),
    external_blocked: count("external_blocked"),
  }
}

/// Repositories with no local checkout.
fn missing(checkouts: &BTreeMap<String, Option<String>>) -> Vec<String> {
  checkouts
    .iter()
    .filter(|(_, path)| path.is_none())
    .map(|(repo, _)| repo.clone())
    .collect()
}

#[cfg(test)]
#[path = "tests/from_tracker_test.rs"]
mod tests;
