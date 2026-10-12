//! Close the delivered item and tick the epic through the epic's tracker
//! (`settleIssue`): the saved graph's tracker, else the one the epic
//! reference names.

use serde_json::{Map, Value, json};
use toolu_runtime::env::Env;

use crate::graph::Graph;
use crate::refs::parse_ref;
use crate::tracker::{Gh, Tracker, TrackerKind, detect_tracker};

/// Builds the tracker of an epic reference.
pub(crate) type MakeTracker<'a> =
  dyn Fn(TrackerKind, &str, &str) -> Result<Box<dyn Tracker + Sync>, String> + 'a;

/// Where a settlement looks things up.
pub(crate) struct Settle<'a> {
  /// The environment, for tracker detection.
  pub env: &'a Env,
  /// GitHub, for a title the graph lacks.
  pub gh: &'a Gh,
  /// The tracker factory.
  pub make: &'a MakeTracker<'a>,
}

/// Close `issue` with a delivery note and tick it on the epic.
///
/// # Errors
/// The tracker cannot be built or refuses.
pub(crate) fn settle_issue(
  at: &Settle<'_>,
  issue: &str,
  pr_url: &str,
  epic: Option<&str>,
  graph: Option<&Graph>,
) -> Result<Map<String, Value>, String> {
  let epic = epic
    .map(str::to_owned)
    .or_else(|| graph.map(|graph| graph.epic.reference.clone()));
  let anchor = epic.as_deref().unwrap_or(issue);
  let kind = detect_tracker(
    anchor,
    graph.and_then(|graph| graph.tracker.as_deref()),
    at.env,
  )?;
  let default_repo = graph
    .and_then(|graph| graph.default_repo.clone())
    .unwrap_or_default();
  let tracker = (at.make)(kind, anchor, &default_repo)?;
  let note = match &epic {
    Some(epic) => format!("Delivered in {pr_url} (epic {epic})."),
    None => format!("Delivered in {pr_url}."),
  };
  let mut out = Map::new();
  out.insert(
    "issue".to_owned(),
    json!(tracker.close_issue(issue, &note)?),
  );
  if epic.is_none() {
    return Ok(out);
  }
  let title = issue_title(at, issue, graph, kind)?;
  out.insert(
    "epic_ticked".to_owned(),
    json!(tracker.tick_epic(issue, &title)?),
  );
  Ok(out)
}

/// The issue's title for the epic checklist: the graph's, else GitHub's.
fn issue_title(
  at: &Settle<'_>,
  issue: &str,
  graph: Option<&Graph>,
  kind: TrackerKind,
) -> Result<String, String> {
  if let Some(found) = graph.and_then(|graph| graph.find(issue)) {
    return Ok(found.title.clone());
  }
  if kind != TrackerKind::Github {
    return Ok(String::new());
  }
  let found = parse_ref(issue, None)?;
  let body: Value = at.gh.get(&format!(
    "/repos/{}/{}/issues/{}",
    found.owner, found.repo, found.number
  ))?;
  Ok(
    body
      .get("title")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned(),
  )
}

#[cfg(test)]
#[path = "tests/settle_test.rs"]
mod tests;
