//! A GitHub epic's children: native sub-issues, else the epic body's task
//! list; each child's blockers (dependency API plus "blocked by" text), labels,
//! body excerpt and closing pull requests.

use std::collections::{BTreeMap, BTreeSet};

use regex::RegexBuilder;
use serde_json::{Value, json};

use super::gh::Gh;
use super::types::{PrNode, TrackedIssue, excerpt, state_word};
use crate::refs::{IssueRef, parse_ref, refs_in};
use crate::retry::map_limit;

/// Concurrent detail fetches: quick, and under GitHub's secondary limit.
pub(super) const FETCH_CONCURRENCY: usize = 4;

const CLOSING_PRS: &str = "query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){issue(number:$n){\nclosedByPullRequestsReferences(first:10,includeClosedPrs:true){nodes{number state url headRefName}}}}}";

/// A child with any details already present in its listing or task-list GET.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SubIssue {
  pub reference: String,
  pub title: String,
  pub state: String,
  pub url: String,
  pub body: Option<String>,
  pub labels: Option<Vec<String>>,
}

/// `owner/repo#N` from an issue's `repository_url` and `number`.
pub(super) fn ref_of(item: &Value) -> Result<String, String> {
  let url = item
    .get("repository_url")
    .and_then(Value::as_str)
    .unwrap_or("");
  let mut parts = url.trim_end_matches('/').rsplit('/');
  let repo = parts.next().filter(|part| !part.is_empty());
  let owner = parts.next().filter(|part| !part.is_empty());
  let number = item.get("number").and_then(Value::as_u64);
  match (owner, repo, number) {
    (Some(owner), Some(repo), Some(number)) => Ok(format!("{owner}/{repo}#{number}")),
    _ => Err(format!("bad repository_url: {url}")),
  }
}

fn text(item: &Value, key: &str) -> String {
  item
    .get(key)
    .and_then(Value::as_str)
    .unwrap_or("")
    .to_owned()
}

fn sub_of(item: &Value, reference: String) -> SubIssue {
  SubIssue {
    reference,
    title: text(item, "title"),
    state: text(item, "state"),
    url: text(item, "html_url"),
    body: item.get("body").and_then(Value::as_str).map(str::to_owned),
    labels: item
      .get("labels")
      .and_then(Value::as_array)
      .map(|_| labels(item)),
  }
}

fn issue_path(issue: &IssueRef) -> String {
  format!(
    "/repos/{}/{}/issues/{}",
    issue.owner, issue.repo, issue.number
  )
}

/// The epic's sub-issues, or the issues its body's task list names.
///
/// # Errors
/// A GitHub call fails.
pub(super) fn sub_issues(gh: &Gh, epic: &IssueRef, body: &str) -> Result<Vec<SubIssue>, String> {
  let path = format!("{}/sub_issues?per_page=100", issue_path(epic));
  let items = gh.get_all(&path)?;
  if !items.is_empty() {
    return items
      .iter()
      .map(|item| ref_of(item).map(|reference| sub_of(item, reference)))
      .collect();
  }
  let repo = format!("{}/{}", epic.owner, epic.repo);
  let task = RegexBuilder::new(r"^\s*[-*]\s*\[[ xX]\]")
    .build()
    .map_err(|err| err.to_string())?;
  let mut seen = BTreeSet::new();
  let mut refs = Vec::new();
  for line in body.lines().filter(|line| task.is_match(line)) {
    for reference in refs_in(line, &repo)? {
      if seen.insert(reference.clone()) {
        refs.push(reference);
      }
    }
  }
  map_limit(&refs, FETCH_CONCURRENCY, |reference| {
    let issue = parse_ref(reference, None)?;
    let item: Value = gh.get(&issue_path(&issue))?;
    Ok(sub_of(&item, reference.clone()))
  })
}

/// Every detail of one child.
///
/// # Errors
/// A GitHub call other than the dependency API fails.
pub(super) fn details(gh: &Gh, sub: &SubIssue) -> Result<TrackedIssue, String> {
  let issue = parse_ref(&sub.reference, None)?;
  let repo = format!("{}/{}", issue.owner, issue.repo);
  let path = issue_path(&issue);
  let mut blockers = BTreeMap::new();
  // Only an absent dependency API falls back to text blockers; a rate limit
  // fails the build rather than lose the native blockers.
  let deps_api =
    match gh.get_if_present::<Vec<Value>>(&format!("{path}/dependencies/blocked_by"))? {
      Some(rows) => {
        for row in rows {
          blockers.insert(ref_of(&row)?, text(&row, "state"));
        }
        true
      }
      None => false,
    };
  let fetched = if sub.body.is_none() || sub.labels.is_none() {
    Some(gh.get::<Value>(&path)?)
  } else {
    None
  };
  let body = sub.body.as_deref().unwrap_or_else(|| {
    fetched
      .as_ref()
      .and_then(|item| item.get("body"))
      .and_then(Value::as_str)
      .unwrap_or("")
  });
  text_blockers(gh, body, &repo, &sub.reference, &mut blockers)?;
  Ok(TrackedIssue {
    reference: sub.reference.clone(),
    title: sub.title.clone(),
    state: state_word(sub.state == "closed"),
    url: sub.url.clone(),
    repo,
    number: Some(issue.number),
    blockers,
    prs: closing_prs(gh, &issue)?,
    deps_source: if deps_api { "native+text" } else { "text" }.to_owned(),
    labels: sub
      .labels
      .clone()
      .unwrap_or_else(|| fetched.as_ref().map_or_else(Vec::new, labels)),
    excerpt: excerpt(body),
  })
}

/// The issues a body's "blocked by" or "depends on" text names, in order,
/// without duplicates or `own`.
///
/// # Errors
/// The pattern fails to compile.
pub(super) fn text_blocker_refs(body: &str, repo: &str, own: &str) -> Result<Vec<String>, String> {
  let lines = RegexBuilder::new(r"\b(?:blocked by|depends on)\b[^\n]*")
    .case_insensitive(true)
    .build()
    .map_err(|err| err.to_string())?;
  let mut refs = Vec::new();
  for found in lines.find_iter(body) {
    for reference in refs_in(found.as_str(), repo)? {
      if reference != own && !refs.contains(&reference) {
        refs.push(reference);
      }
    }
  }
  Ok(refs)
}

fn text_blockers(
  gh: &Gh,
  body: &str,
  repo: &str,
  own: &str,
  blockers: &mut BTreeMap<String, String>,
) -> Result<(), String> {
  for reference in text_blocker_refs(body, repo, own)? {
    if blockers.contains_key(&reference) {
      continue;
    }
    let issue = parse_ref(&reference, None)?;
    let item: Value = gh.get(&issue_path(&issue))?;
    blockers.insert(reference, text(&item, "state"));
  }
  Ok(())
}

fn closing_prs(gh: &Gh, issue: &IssueRef) -> Result<Vec<PrNode>, String> {
  let data = gh.graphql(
    CLOSING_PRS,
    &json!({"o": issue.owner, "r": issue.repo, "n": issue.number}),
    false,
  )?;
  let nodes = data
    .pointer("/repository/issue/closedByPullRequestsReferences/nodes")
    .cloned()
    .unwrap_or(Value::Array(Vec::new()));
  serde_json::from_value(nodes).map_err(|err| format!("closing PRs: {err}"))
}

/// Label names, whether GitHub sent objects or strings.
pub(super) fn labels(item: &Value) -> Vec<String> {
  item
    .get("labels")
    .and_then(Value::as_array)
    .map(|rows| {
      rows
        .iter()
        .filter_map(|row| {
          row
            .as_str()
            .or_else(|| row.get("name").and_then(Value::as_str))
        })
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
    })
    .unwrap_or_default()
}

#[cfg(test)]
#[path = "tests/github_fetch_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/github_fetch_reuse_test.rs"]
mod reuse_tests;

#[cfg(test)]
#[path = "tests/epic_creator_bodies_test.rs"]
mod epic_creator_tests;
