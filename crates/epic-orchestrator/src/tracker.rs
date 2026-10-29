//! Where an epic's plan lives (`scripts/trackers/*`): GitHub, Jira or Linear.
//! The code, branches and pull requests always live in GitHub repositories.

use std::path::Path;

use toolu_runtime::env::Env;

mod close_intent;
mod detect;
mod gh;
/// GitHub tracker adapter.
pub(crate) mod github;
mod github_body;
mod github_fetch;
/// Jira tracker adapter.
pub(crate) mod jira;
/// Jira REST client.
pub(crate) mod jira_client;
mod linear;
mod linear_parse;
/// Shared JSON HTTP transport for Jira and Linear.
pub(crate) mod rest;
mod types;

pub(crate) use detect::{TrackerKind, detect_tracker, parse_jira_ref};
pub(crate) use gh::Gh;
pub(crate) use types::{EpicInfo, PrNode, TrackedIssue};

/// One epic in its tracker.
pub(crate) trait Tracker {
  /// The tracker this epic lives in.
  fn kind(&self) -> TrackerKind;
  /// The epic's title, state and URL.
  ///
  /// # Errors
  /// The tracker call fails.
  fn epic(&self) -> Result<EpicInfo, String>;
  /// Every child work item, normalized.
  ///
  /// # Errors
  /// A tracker call fails.
  fn children(&self) -> Result<Vec<TrackedIssue>, String>;
  /// Whether a child is closed (done) in the tracker.
  ///
  /// # Errors
  /// The tracker call fails.
  fn is_closed(&self, reference: &str) -> Result<bool, String>;
  /// Mark a delivered child done; returns how it ended up closed.
  ///
  /// # Errors
  /// A tracker call fails.
  fn close_issue(&self, reference: &str, note: &str, state_dir: &Path) -> Result<String, String>;
  /// Tick the child in the epic body checklist, where the tracker has one.
  ///
  /// # Errors
  /// A tracker call fails.
  fn tick_epic(&self, reference: &str, title: &str) -> Result<bool, String>;
  /// Comment the summary on the epic and close it.
  ///
  /// # Errors
  /// A tracker call fails.
  fn close_epic(&self, summary: &str, state_dir: &Path) -> Result<String, String>;
  /// The epic's state directory name.
  fn state_slug(&self) -> String;
}

/// The tracker for `reference` of `kind`. `default_repo` holds the code of
/// Jira and Linear children without a `repo:` label or `Repo:` line.
///
/// # Errors
/// The reference does not fit the tracker, or its credentials are missing.
pub(crate) fn make_tracker(
  kind: TrackerKind,
  reference: &str,
  default_repo: &str,
  env: &Env,
) -> Result<Box<dyn Tracker + Sync>, String> {
  Ok(match kind {
    TrackerKind::Github => Box::new(github::GitHubTracker::new(reference, env)?),
    TrackerKind::Jira => Box::new(jira::JiraTracker::new(reference, default_repo, env)?),
    TrackerKind::Linear => Box::new(linear::LinearTracker::new(reference, default_repo, env)?),
  })
}

#[cfg(test)]
#[path = "tests/tracker_test.rs"]
mod tests;
