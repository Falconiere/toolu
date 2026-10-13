//! Which journal records belong to the measured epic, learned in a first
//! pass: a record is in epic `E` when it names `E`, when its key is an issue
//! seen with `E`, or when it is a GitHub attempt on a pull request of such an
//! issue. Machine-wide records with no key count in every scope.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::journal::Record;

/// Names of keyless records that describe the whole engine.
const MACHINE_WIDE: [&str; 2] = ["wait-reply", "engine-usage"];

/// The epic filter and the issue facts the fold needs.
#[derive(Debug, Default)]
pub(crate) struct Scope {
  epic: Option<String>,
  issue_epic: BTreeMap<String, String>,
  watch_issue: BTreeMap<String, String>,
  issue_repo: BTreeMap<String, String>,
}

impl Scope {
  /// An empty scope for `epic`, or for every epic.
  pub(crate) fn new(epic: Option<&str>) -> Self {
    Self {
      epic: epic.map(str::to_owned),
      ..Self::default()
    }
  }

  /// Learn the epic of an issue key and the issue and repository of a watch.
  pub(crate) fn learn(&mut self, record: &Record) {
    if !record.epic.is_empty() && !record.key.is_empty() && record.epic != record.key {
      self
        .issue_epic
        .entry(record.key.clone())
        .or_insert_with(|| record.epic.clone());
    }
    if let Some(issue) = github_issue(record) {
      if let Some(repo) = watch_repo(&record.key) {
        self.issue_repo.entry(issue.clone()).or_insert(repo);
      }
      self.watch_issue.insert(record.key.clone(), issue);
    }
  }

  /// Whether `record` is in scope.
  pub(crate) fn keeps(&self, record: &Record) -> bool {
    let Some(epic) = &self.epic else {
      return true;
    };
    if record.key.is_empty() {
      return MACHINE_WIDE.contains(&record.name.as_str());
    }
    if &record.epic == epic {
      return true;
    }
    self.issue(&record.key).is_some_and(|issue| {
      issue == epic.as_str() || self.issue_epic.get(issue).is_some_and(|seen| seen == epic)
    })
  }

  /// The issue key behind `key`: the issue of a watch key, else `key`.
  pub(crate) fn issue<'a>(&'a self, key: &'a str) -> Option<&'a str> {
    match self.watch_issue.get(key) {
      Some(issue) => Some(issue),
      None if key.starts_with("pr:") => None,
      None => Some(key),
    }
  }

  /// The repository of an issue's pull request, when a check named it.
  pub(crate) fn repo(&self, issue: &str) -> Option<&str> {
    self.issue_repo.get(issue).map(String::as_str)
  }
}

/// The `issue` of a `github-check` or `github-error` note.
fn github_issue(record: &Record) -> Option<String> {
  if record.name != "github-check" && record.name != "github-error" {
    return None;
  }
  let note: Value = serde_json::from_str(&record.note).ok()?;
  note.get("issue")?.as_str().map(str::to_owned)
}

/// `owner/repo` of a `pr:<owner/repo>#<n>` watch key.
fn watch_repo(key: &str) -> Option<String> {
  let rest = key.strip_prefix("pr:")?;
  rest.rsplit_once('#').map(|(repo, _)| repo.to_owned())
}

#[cfg(test)]
#[path = "tests/scope_test.rs"]
mod tests;
