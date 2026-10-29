//! GitHub epics (`scripts/trackers/github.ts`).

use std::path::Path;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_github::Method;
use toolu_runtime::env::Env;

use super::Tracker;
use super::close_intent;
use super::detect::TrackerKind;
use super::gh::Gh;
use super::github_body::tick_body;
use super::github_fetch::{FETCH_CONCURRENCY, details, sub_issues};
use super::types::{EpicInfo, TrackedIssue, state_word};
use crate::refs::{IssueRef, parse_ref};
use crate::retry::map_limit;

/// A GitHub epic and its client.
pub(crate) struct GitHubTracker {
  epic: IssueRef,
  reference: String,
  gh: Gh,
  body: Mutex<Option<String>>,
  polls: ClosePolls,
}

/// How often `close_issue` looks for the merged PR's own close first.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClosePolls {
  /// Looks before closing by hand.
  pub count: u32,
  /// The wait between looks.
  pub wait: Duration,
}

/// Five looks three seconds apart, as `closeIssue` does.
pub(crate) const CLOSE_POLLS: ClosePolls = ClosePolls {
  count: 5,
  wait: Duration::from_secs(3),
};

impl GitHubTracker {
  /// The epic `reference` names.
  ///
  /// # Errors
  /// The reference is malformed or no GitHub token is available.
  pub(crate) fn new(reference: &str, env: &Env) -> Result<Self, String> {
    Self::with(reference, Gh::new(env)?, CLOSE_POLLS)
  }

  /// The epic over an existing client; tests pass a fixture client.
  ///
  /// # Errors
  /// The reference is malformed.
  pub(crate) fn with(reference: &str, gh: Gh, polls: ClosePolls) -> Result<Self, String> {
    let epic = parse_ref(reference, None)?;
    Ok(Self {
      reference: epic.canonical(),
      epic,
      gh,
      body: Mutex::new(None),
      polls,
    })
  }

  fn path(&self) -> String {
    format!(
      "/repos/{}/{}/issues/{}",
      self.epic.owner, self.epic.repo, self.epic.number
    )
  }

  fn epic_body(&self) -> Result<String, String> {
    if let Ok(guard) = self.body.lock()
      && let Some(body) = guard.as_ref()
    {
      return Ok(body.clone());
    }
    let item: Value = self.gh.get(&self.path())?;
    Ok(
      item
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned(),
    )
  }

  fn issue_state(&self, issue: &IssueRef) -> Result<String, String> {
    let path = format!(
      "/repos/{}/{}/issues/{}",
      issue.owner, issue.repo, issue.number
    );
    let item: Value = self.gh.get(&path)?;
    Ok(
      item
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned(),
    )
  }

  fn has_comment(&self, path: &str, note: &str) -> Result<bool, String> {
    for row in self.gh.get_all(&format!("{path}/comments?per_page=100"))? {
      let body = row
        .get("body")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("GitHub {path}: invalid comment response"))?;
      if body == note {
        return Ok(true);
      }
    }
    Ok(false)
  }

  fn close_with_note(
    &self,
    issue: &IssueRef,
    role: &str,
    note: &str,
    state_dir: &Path,
  ) -> Result<(), String> {
    let path = format!(
      "/repos/{}/{}/issues/{}",
      issue.owner, issue.repo, issue.number
    );
    close_intent::comment(
      state_dir,
      ("github", role, &issue.canonical()),
      note,
      |body| self.has_comment(&path, body),
      |body| {
        self.gh.write(
          Method::Post,
          &format!("{path}/comments"),
          &json!({"body": body}),
        )?;
        Ok(())
      },
    )?;
    self.gh.write(
      Method::Patch,
      &path,
      &json!({"state": "closed", "state_reason": "completed"}),
    )?;
    Ok(())
  }
}

impl Tracker for GitHubTracker {
  fn kind(&self) -> TrackerKind {
    TrackerKind::Github
  }

  fn epic(&self) -> Result<EpicInfo, String> {
    let item: Value = self.gh.get(&self.path())?;
    let body = item
      .get("body")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned();
    if let Ok(mut guard) = self.body.lock() {
      *guard = Some(body);
    }
    let text = |key: &str| {
      item
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
    };
    Ok(EpicInfo {
      reference: self.reference.clone(),
      title: text("title"),
      state: state_word(text("state") == "closed"),
      url: text("html_url"),
    })
  }

  fn children(&self) -> Result<Vec<TrackedIssue>, String> {
    let subs = sub_issues(&self.gh, &self.epic, &self.epic_body()?)?;
    map_limit(&subs, FETCH_CONCURRENCY, |sub| details(&self.gh, sub))
  }

  fn is_closed(&self, reference: &str) -> Result<bool, String> {
    Ok(self.issue_state(&parse_ref(reference, None)?)? == "closed")
  }

  fn close_issue(&self, reference: &str, note: &str, state_dir: &Path) -> Result<String, String> {
    let issue = parse_ref(reference, None)?;
    for poll in 0..self.polls.count {
      if self.issue_state(&issue)? == "closed" {
        return Ok("closed-by-pr".to_owned());
      }
      if poll + 1 < self.polls.count {
        thread::sleep(self.polls.wait);
      }
    }
    self.close_with_note(&issue, "issue", note, state_dir)?;
    Ok("closed-manually".to_owned())
  }

  fn tick_epic(&self, reference: &str, title: &str) -> Result<bool, String> {
    let item: Value = self.gh.get(&self.path())?;
    let body = item.get("body").and_then(Value::as_str).unwrap_or("");
    let next = tick_body(body, (&self.epic.owner, &self.epic.repo), reference, title)?;
    if next == body {
      return Ok(false);
    }
    self
      .gh
      .write(Method::Patch, &self.path(), &json!({"body": next}))?;
    Ok(true)
  }

  fn close_epic(&self, summary: &str, state_dir: &Path) -> Result<String, String> {
    if self.issue_state(&self.epic)? == "closed" {
      return Ok("already-done".to_owned());
    }
    self.close_with_note(&self.epic, "epic", summary, state_dir)?;
    Ok("closed".to_owned())
  }

  fn state_slug(&self) -> String {
    format!(
      "{}-{}-{}",
      self.epic.owner, self.epic.repo, self.epic.number
    )
    .to_lowercase()
  }
}

#[cfg(test)]
#[path = "tests/github_test.rs"]
mod tests;
