//! Jira epics (`scripts/trackers/jira.ts`). Children are `parent = EPIC`
//! (Cloud and team-managed) or `"Epic Link" = EPIC` (Server/DC); blockers are
//! links whose wording from the child's side is "is blocked by" or "depends on".

use std::collections::BTreeMap;
use std::path::Path;

use regex::RegexBuilder;
use serde_json::Value;
use toolu_runtime::env::Env;

use super::Tracker;
use super::close_intent;
use super::detect::{TrackerKind, parse_jira_ref};
use super::jira_client::JiraClient;
use super::types::{EpicInfo, TrackedIssue, excerpt, repo_for, state_word};

/// A Jira epic and its client.
pub(crate) struct JiraTracker {
  reference: String,
  default_repo: String,
  client: JiraClient,
}

impl JiraTracker {
  /// The epic `reference` names, with credentials from `env`.
  ///
  /// # Errors
  /// The reference is no Jira key or browse URL, or a setting is missing.
  pub(crate) fn new(reference: &str, default_repo: &str, env: &Env) -> Result<Self, String> {
    Self::with(reference, default_repo, JiraClient::new(env)?)
  }

  /// The epic over an existing client.
  ///
  /// # Errors
  /// The reference is no Jira key or browse URL.
  pub(crate) fn with(
    reference: &str,
    default_repo: &str,
    client: JiraClient,
  ) -> Result<Self, String> {
    let key = parse_jira_ref(reference)
      .ok_or_else(|| format!("not a Jira issue key or browse URL: {reference}"))?;
    Ok(Self {
      reference: key,
      default_repo: default_repo.to_owned(),
      client,
    })
  }

  fn child(&self, issue: &Value) -> TrackedIssue {
    let key = issue
      .get("key")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned();
    let fields = issue.get("fields").cloned().unwrap_or(Value::Null);
    let labels: Vec<String> = fields
      .get("labels")
      .and_then(Value::as_array)
      .map(|rows| {
        rows
          .iter()
          .filter_map(Value::as_str)
          .map(str::to_owned)
          .collect()
      })
      .unwrap_or_default();
    let body = adf_text(fields.get("description").unwrap_or(&Value::Null));
    TrackedIssue {
      title: summary(&fields, &key),
      state: state_word(done(fields.get("status"))),
      url: format!("{}/browse/{key}", self.client.base),
      repo: repo_for(&labels, &body, &self.default_repo),
      number: None,
      blockers: jira_blockers(fields.get("issuelinks")),
      prs: Vec::new(),
      deps_source: "links".to_owned(),
      excerpt: excerpt(&body),
      labels,
      reference: key,
    }
  }

  /// Move to the first transition that lands in the Done category.
  fn transition_done(&self, key: &str) -> Result<bool, String> {
    let target = self.client.transitions(key)?.into_iter().find(|row| {
      row
        .pointer("/to/statusCategory/key")
        .and_then(Value::as_str)
        == Some("done")
    });
    let Some(name) =
      target.and_then(|row| row.get("name").and_then(Value::as_str).map(str::to_owned))
    else {
      return Ok(false);
    };
    self.client.transition(key, &name)?;
    Ok(true)
  }
}

impl Tracker for JiraTracker {
  fn kind(&self) -> TrackerKind {
    TrackerKind::Jira
  }

  fn epic(&self) -> Result<EpicInfo, String> {
    let issue = self.client.issue(&self.reference, &["summary", "status"])?;
    let fields = issue.get("fields").cloned().unwrap_or(Value::Null);
    Ok(EpicInfo {
      reference: self.reference.clone(),
      title: summary(&fields, &self.reference),
      state: state_word(done(fields.get("status"))),
      url: format!("{}/browse/{}", self.client.base, self.reference),
    })
  }

  fn children(&self) -> Result<Vec<TrackedIssue>, String> {
    let mut items = self
      .client
      .search(&format!("parent = {} ORDER BY key", self.reference))?;
    if items.is_empty() {
      match self
        .client
        .search(&format!("\"Epic Link\" = {} ORDER BY key", self.reference))
      {
        Ok(found) => items = found,
        // Cloud rejects the retired Epic Link field; parent was the answer.
        Err(err) if err.contains("HTTP 400") => {}
        Err(err) => return Err(err),
      }
    }
    Ok(items.iter().map(|issue| self.child(issue)).collect())
  }

  fn is_closed(&self, reference: &str) -> Result<bool, String> {
    let issue = self.client.issue(reference, &["status"])?;
    Ok(done(issue.pointer("/fields/status")))
  }

  fn close_issue(&self, reference: &str, note: &str, state_dir: &Path) -> Result<String, String> {
    if self.is_closed(reference)? {
      return Ok("already-done".to_owned());
    }
    close_intent::comment(
      state_dir,
      ("jira", "issue", reference),
      note,
      |body| self.client.has_comment(reference, body),
      |body| self.client.comment(reference, body),
    )?;
    Ok(done_word(self.transition_done(reference)?))
  }

  fn tick_epic(&self, _reference: &str, _title: &str) -> Result<bool, String> {
    // Jira tracks children through the parent field, not a body checklist.
    Ok(false)
  }

  fn close_epic(&self, summary: &str, state_dir: &Path) -> Result<String, String> {
    if self.is_closed(&self.reference)? {
      return Ok("already-done".to_owned());
    }
    close_intent::comment(
      state_dir,
      ("jira", "epic", &self.reference),
      summary,
      |body| self.client.has_comment(&self.reference, body),
      |body| self.client.comment(&self.reference, body),
    )?;
    Ok(done_word(self.transition_done(&self.reference)?))
  }

  fn state_slug(&self) -> String {
    format!("jira-{}", self.reference).to_lowercase()
  }
}

fn done_word(moved: bool) -> String {
  if moved {
    "transitioned-done"
  } else {
    "no-done-transition"
  }
  .to_owned()
}

fn summary(fields: &Value, fallback: &str) -> String {
  fields
    .get("summary")
    .and_then(Value::as_str)
    .unwrap_or(fallback)
    .to_owned()
}

/// A status in the Done category.
fn done(status: Option<&Value>) -> bool {
  status
    .and_then(|status| status.pointer("/statusCategory/key"))
    .and_then(Value::as_str)
    == Some("done")
}

/// Blockers of one issue from its links, keyed by issue key.
pub(crate) fn jira_blockers(links: Option<&Value>) -> BTreeMap<String, String> {
  let mut out = BTreeMap::new();
  let Ok(blocking) = RegexBuilder::new(r"\bblocked by\b|\bdepends on\b")
    .case_insensitive(true)
    .build()
  else {
    return out;
  };
  for link in links.and_then(Value::as_array).into_iter().flatten() {
    let (other, side) = match (link.get("inwardIssue"), link.get("outwardIssue")) {
      (Some(inward), _) => (inward, "inward"),
      (None, Some(outward)) => (outward, "outward"),
      (None, None) => continue,
    };
    let wording = link
      .pointer(&format!("/type/{side}"))
      .and_then(Value::as_str)
      .unwrap_or("");
    if let Some(key) = other.get("key").and_then(Value::as_str)
      && blocking.is_match(wording)
    {
      out.insert(
        key.to_owned(),
        state_word(done(other.pointer("/fields/status"))),
      );
    }
  }
  out
}

/// Plain text from an Atlassian Document Format node (v3) or a v2 string.
pub(crate) fn adf_text(node: &Value) -> String {
  match node {
    Value::String(text) => text.clone(),
    Value::Object(map) => {
      let text = map.get("text").and_then(Value::as_str).unwrap_or("");
      let kind = map.get("type").and_then(Value::as_str).unwrap_or("");
      let inner: Vec<String> = map
        .get("content")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().map(adf_text).collect())
        .unwrap_or_default();
      let joined = inner.join(if kind == "doc" { "\n" } else { "" });
      let tail = if kind == "paragraph" { "\n" } else { "" };
      format!("{text}{joined}{tail}")
    }
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) => String::new(),
  }
}

#[cfg(test)]
#[path = "tests/jira_test.rs"]
mod tests;
