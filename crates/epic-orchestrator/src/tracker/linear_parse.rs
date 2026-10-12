//! Linear GraphQL payloads: child nodes, workflow states, inverse `blocks`
//! relations, and the `Authorization` value each key type sends.

use std::collections::BTreeMap;

use serde_json::Value;

use super::types::{TrackedIssue, excerpt, repo_for, state_word};

/// One child issue from its GraphQL node.
pub(super) fn child(issue: &Value, default_repo: &str) -> TrackedIssue {
  let text = |key: &str| {
    issue
      .get(key)
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned()
  };
  let labels: Vec<String> = issue
    .pointer("/labels/nodes")
    .and_then(Value::as_array)
    .map(|rows| {
      rows
        .iter()
        .filter_map(|row| row.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
    })
    .unwrap_or_default();
  let body = text("description");
  TrackedIssue {
    reference: text("identifier"),
    title: text("title"),
    state: state_word(closed(issue.get("state"))),
    url: text("url"),
    repo: repo_for(&labels, &body, default_repo),
    number: None,
    blockers: linear_blockers(issue),
    prs: Vec::new(),
    deps_source: "relations".to_owned(),
    labels,
    excerpt: excerpt(&body),
  }
}

/// A personal key (`lin_api_…`) goes bare; an OAuth token (`lin_oauth_…`) as `Bearer`.
pub(crate) fn linear_auth_header(token: &str) -> String {
  let token = token.trim();
  let bearer = token
    .get(..6)
    .is_some_and(|head| head.eq_ignore_ascii_case("bearer"))
    && token
      .get(6..)
      .is_some_and(|rest| rest.starts_with(char::is_whitespace));
  if bearer {
    token.to_owned()
  } else if token.starts_with("lin_oauth_") {
    format!("Bearer {token}")
  } else {
    token.to_owned()
  }
}

/// A `completed` or `canceled` workflow state.
pub(super) fn closed(state: Option<&Value>) -> bool {
  matches!(
    state
      .and_then(|state| state.get("type"))
      .and_then(Value::as_str),
    Some("completed" | "canceled")
  )
}

/// Blockers from inverse `blocks` relations only.
pub(crate) fn linear_blockers(issue: &Value) -> BTreeMap<String, String> {
  let mut out = BTreeMap::new();
  for relation in issue
    .pointer("/inverseRelations/nodes")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
  {
    if relation.get("type").and_then(Value::as_str) != Some("blocks") {
      continue;
    }
    if let Some(identifier) = relation
      .pointer("/issue/identifier")
      .and_then(Value::as_str)
    {
      out.insert(
        identifier.to_owned(),
        state_word(closed(relation.pointer("/issue/state"))),
      );
    }
  }
  out
}

/// The id of the team's completed state with the lowest position.
pub(super) fn lowest_completed(issue: &Value) -> Option<String> {
  let position = |row: &&Value| {
    row
      .get("position")
      .and_then(Value::as_f64)
      .unwrap_or(f64::MAX)
  };
  issue
    .pointer("/team/states/nodes")
    .and_then(Value::as_array)?
    .iter()
    .min_by(|left, right| position(left).total_cmp(&position(right)))
    .and_then(|row| row.get("id").and_then(Value::as_str).map(str::to_owned))
}

#[cfg(test)]
#[path = "tests/linear_parse_test.rs"]
mod tests;
