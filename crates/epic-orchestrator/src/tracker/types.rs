//! Tracker-neutral shapes (`scripts/trackers/types.ts`).

use std::collections::BTreeMap;

use regex::RegexBuilder;
use serde::{Deserialize, Serialize};

/// A pull request that closes a child.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PrNode {
  /// The PR number.
  pub number: u64,
  /// `OPEN`, `MERGED` or `CLOSED`.
  pub state: String,
  /// The PR URL.
  pub url: String,
  /// The head branch.
  #[serde(rename = "headRefName")]
  pub head_ref_name: String,
}

/// The epic itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EpicInfo {
  /// Canonical reference.
  #[serde(rename = "ref")]
  pub reference: String,
  /// Title.
  pub title: String,
  /// `open` or `closed`.
  pub state: String,
  /// Web URL.
  pub url: String,
}

/// One child work item, normalized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TrackedIssue {
  /// `owner/repo#N` (GitHub), `ABC-12` (Jira) or `ENG-12` (Linear).
  #[serde(rename = "ref")]
  pub reference: String,
  /// Title.
  pub title: String,
  /// `open` or `closed`.
  pub state: String,
  /// Web URL.
  pub url: String,
  /// The GitHub repository that holds the code and the PR.
  pub repo: String,
  /// The GitHub issue number; `None` for Jira and Linear items.
  pub number: Option<u64>,
  /// Blocker reference to its state (`open` or `closed`).
  pub blockers: BTreeMap<String, String>,
  /// PRs that close it.
  pub prs: Vec<PrNode>,
  /// Where the blockers came from.
  pub deps_source: String,
  /// Labels.
  pub labels: Vec<String>,
  /// The leading slice of the description, for complexity routing.
  pub excerpt: String,
}

/// How much of a description routing and coupling see.
pub(crate) const EXCERPT_CHARS: usize = 1500;

/// The first [`EXCERPT_CHARS`] characters of the trimmed text.
pub(crate) fn excerpt(text: &str) -> String {
  text.trim().chars().take(EXCERPT_CHARS).collect()
}

/// `open` or `closed`.
pub(crate) fn state_word(closed: bool) -> String {
  if closed { "closed" } else { "open" }.to_owned()
}

/// The code repository of a Jira or Linear item: a `repo:owner/name` label,
/// then a `Repo: owner/name` line in its description, then `fallback`.
pub(crate) fn repo_for(labels: &[String], body: &str, fallback: &str) -> String {
  let label = RegexBuilder::new(r"^repo:([\w.-]+/[\w.-]+)$")
    .case_insensitive(true)
    .build();
  let line = RegexBuilder::new(r"^\s*repo:\s*([\w.-]+/[\w.-]+)\s*$")
    .case_insensitive(true)
    .multi_line(true)
    .build();
  if let Ok(label) = label {
    for name in labels {
      if let Some(found) = label.captures(name.trim()).and_then(|caps| caps.get(1)) {
        return found.as_str().to_owned();
      }
    }
  }
  line
    .ok()
    .and_then(|line| line.captures(body))
    .and_then(|caps| caps.get(1))
    .map_or_else(|| fallback.to_owned(), |found| found.as_str().to_owned())
}

#[cfg(test)]
#[path = "tests/types_test.rs"]
mod tests;
