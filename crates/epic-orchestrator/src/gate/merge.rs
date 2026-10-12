//! A merge pinned to the verified head (`doMerge`): the plain merge first,
//! administrator privileges only after a protection-only refusal, an
//! uncertain (timed-out) merge reported and never retried.

use std::time::Duration;

use regex::RegexBuilder;
use serde_json::{Map, Value, json};
use toolu_github::Method;

use super::assess::{Facts, Pr};
use crate::tracker::Gh;

/// Refusals that only branch protection explains, so `--admin` may follow.
pub(crate) const PROTECTION: &str = "protected branch|branch policy|required status|review is required|approving review|not mergeable: the base branch|merge requirements|--admin";

/// How one merge attempt ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Attempt {
  /// GitHub merged.
  Merged,
  /// Refused: the HTTP status (0 before any request) and GitHub's message.
  Refused(u16, String),
  /// The effect is unknown: the request timed out.
  Uncertain(String),
}

/// `success`, `admin`, `failed` or `uncertain`.
pub(crate) fn outcome(attempt: &Attempt) -> &'static str {
  match attempt {
    Attempt::Merged => "success",
    Attempt::Uncertain(_) => "uncertain",
    Attempt::Refused(_, message) => {
      let protected = RegexBuilder::new(PROTECTION)
        .case_insensitive(true)
        .build()
        .is_ok_and(|re| re.is_match(message));
      if protected { "admin" } else { "failed" }
    }
  }
}

/// The squash, merge or rebase method the repository allows, unless given.
///
/// # Errors
/// The repository cannot be read.
pub(crate) fn merge_method(gh: &Gh, pr: &Pr, wanted: Option<&str>) -> Result<String, String> {
  if let Some(method) = wanted {
    return Ok(method.to_owned());
  }
  let repo: Value = gh.get(&format!("/repos/{}", pr.slug()))?;
  let allowed = |key: &str| repo.get(key) == Some(&Value::Bool(true));
  Ok(
    if allowed("allow_squash_merge") {
      "squash"
    } else if allowed("allow_merge_commit") {
      "merge"
    } else {
      "rebase"
    }
    .to_owned(),
  )
}

/// What a merge targets.
pub(crate) struct Target<'a> {
  /// The PR.
  pub pr: &'a Pr,
  /// Its node id and merge state.
  pub facts: &'a Facts,
  /// The verified head.
  pub head: &'a str,
  /// The merge method.
  pub method: &'a str,
}

fn attempt(gh: &Gh, target: &Target<'_>, admin: bool) -> Attempt {
  let (pr, facts, head, method) = (target.pr, target.facts, target.head, target.method);
  if !admin && facts.merge_state == "BLOCKED" {
    let message = format!(
      "Pull request {}#{} is not mergeable: the base branch policy prohibits the merge.",
      pr.slug(),
      pr.number
    );
    return Attempt::Refused(0, message);
  }
  let path = format!("/repos/{}/pulls/{}/merge", pr.slug(), pr.number);
  match gh.write_once(
    Method::Put,
    &path,
    &json!({"merge_method": method, "sha": head}),
  ) {
    Ok(_) => Attempt::Merged,
    Err(toolu_github::Error::Status { status, message }) => Attempt::Refused(status, message),
    Err(toolu_github::Error::Transport(toolu_http::Error::Timeout)) => {
      Attempt::Uncertain("merge command timed out".to_owned())
    }
    Err(err) => Attempt::Refused(0, err.to_string()),
  }
}

fn failure(attempt: &Attempt, admin: bool) -> Map<String, Value> {
  let (status, code, error, uncertain) = match attempt {
    Attempt::Refused(status, message) => (*status, "exit", message.clone(), false),
    Attempt::Uncertain(message) => (0, "timeout", message.clone(), true),
    Attempt::Merged => (0, "exit", String::new(), false),
  };
  let mut out = Map::new();
  out.insert("merged".to_owned(), json!(false));
  if admin {
    out.insert("admin_used".to_owned(), json!(true));
  }
  out.insert("native_exit_code".to_owned(), json!(status));
  out.insert("native_error_code".to_owned(), json!(code));
  out.insert("uncertain".to_owned(), json!(uncertain));
  out.insert("error".to_owned(), json!(error));
  out
}

/// Merge `pr` at `head`, then wait up to 30 s for GitHub to report it merged.
pub(crate) fn do_merge(gh: &Gh, target: &Target<'_>) -> Map<String, Value> {
  let (pr, method) = (target.pr, target.method);
  let first = attempt(gh, target, false);
  let admin = match outcome(&first) {
    "success" => false,
    "admin" => {
      let second = attempt(gh, target, true);
      if outcome(&second) != "success" {
        return failure(&second, true);
      }
      true
    }
    _ => return failure(&first, false),
  };
  for poll in 0..15 {
    let merged = gh
      .get::<Value>(&format!("/repos/{}/pulls/{}", pr.slug(), pr.number))
      .is_ok_and(|body| body.get("merged") == Some(&Value::Bool(true)));
    if merged {
      let mut out = Map::new();
      out.insert("merged".to_owned(), json!(true));
      out.insert("admin_used".to_owned(), json!(admin));
      out.insert("method".to_owned(), json!(method));
      return out;
    }
    if poll < 14 {
      std::thread::sleep(Duration::from_secs(2));
    }
  }
  let mut out = Map::new();
  out.insert("merged".to_owned(), json!(false));
  out.insert("admin_used".to_owned(), json!(admin));
  out.insert(
    "error".to_owned(),
    json!("merge command succeeded but PR is not MERGED after 30s"),
  );
  out
}

#[cfg(test)]
#[path = "tests/merge_test.rs"]
mod tests;
