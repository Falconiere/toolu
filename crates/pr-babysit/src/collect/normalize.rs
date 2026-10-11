//! GitHub's replies reshaped into the snapshot's fields, as `gh pr view` and
//! the shipped collector shaped them: a missing field reads as `null`.

use serde_json::{Value, json};

use crate::json::array;
use crate::reduce::threads::is_ci_reviewer;
use crate::verdict;

fn pick(raw: &Value, pointer: &str) -> Value {
  raw.pointer(pointer).cloned().unwrap_or(Value::Null)
}

/// One review-thread comment.
pub(crate) fn thread_comment(raw: &Value) -> Value {
  json!({
    "id": pick(raw, "/id"),
    "databaseId": pick(raw, "/databaseId"),
    "body": pick(raw, "/body"),
    "author": pick(raw, "/author/login"),
    "authorType": pick(raw, "/author/__typename"),
    "createdAt": pick(raw, "/createdAt"),
    "url": pick(raw, "/url"),
  })
}

/// A review thread and, when its comments overflowed the first page, the
/// cursor after them.
pub(crate) fn thread(raw: &Value) -> (Value, Option<Value>) {
  let comments: Vec<Value> = array(raw.pointer("/comments/nodes"))
    .iter()
    .map(thread_comment)
    .collect();
  let overflow = raw.pointer("/comments/pageInfo/hasNextPage") == Some(&Value::Bool(true));
  let doc = json!({
    "id": pick(raw, "/id"),
    "isResolved": pick(raw, "/isResolved"),
    "isOutdated": pick(raw, "/isOutdated"),
    "path": pick(raw, "/path"),
    "line": pick(raw, "/line"),
    "comments": comments,
  });
  (
    doc,
    overflow.then(|| pick(raw, "/comments/pageInfo/endCursor")),
  )
}

/// One issue (conversation) comment from the REST API.
pub(crate) fn comment(raw: &Value) -> Value {
  json!({
    "id": pick(raw, "/id"),
    "body": pick(raw, "/body"),
    "author": pick(raw, "/user/login"),
    "authorType": pick(raw, "/user/type"),
    "createdAt": pick(raw, "/created_at"),
    "updatedAt": pick(raw, "/updated_at"),
    "url": pick(raw, "/html_url"),
  })
}

/// One pull-request review from the REST API.
pub(crate) fn review(raw: &Value) -> Value {
  json!({
    "id": pick(raw, "/id"),
    "state": pick(raw, "/state"),
    "body": pick(raw, "/body"),
    "author": pick(raw, "/user/login"),
    "authorType": pick(raw, "/user/type"),
    "submittedAt": pick(raw, "/submitted_at"),
    "url": pick(raw, "/html_url"),
    "commitId": pick(raw, "/commit_id"),
  })
}

/// One status-check context as `gh pr view --json statusCheckRollup` prints it.
pub(crate) fn check(raw: &Value) -> Value {
  if raw.get("__typename").and_then(Value::as_str) == Some("StatusContext") {
    return json!({
      "__typename": "StatusContext",
      "context": pick(raw, "/context"),
      "startedAt": pick(raw, "/createdAt"),
      "state": pick(raw, "/state"),
      "targetUrl": pick(raw, "/targetUrl"),
    });
  }
  let workflow = raw
    .pointer("/checkSuite/workflowRun/workflow/name")
    .cloned()
    .unwrap_or_else(|| Value::from(""));
  json!({
    "__typename": pick(raw, "/__typename"),
    "completedAt": pick(raw, "/completedAt"),
    "conclusion": pick(raw, "/conclusion"),
    "detailsUrl": pick(raw, "/detailsUrl"),
    "name": pick(raw, "/name"),
    "startedAt": pick(raw, "/startedAt"),
    "status": pick(raw, "/status"),
    "workflowName": workflow,
  })
}

/// The snapshot's `pr` block from the GraphQL `pullRequest` and its checks.
pub(crate) fn pr(raw: &Value, checks: &[Value]) -> Value {
  let decision = raw
    .get("reviewDecision")
    .filter(|value| !value.is_null())
    .cloned()
    .unwrap_or_else(|| Value::from(""));
  json!({
    "number": pick(raw, "/number"),
    "title": pick(raw, "/title"),
    "url": pick(raw, "/url"),
    "author": pick(raw, "/author/login"),
    "state": pick(raw, "/state"),
    "baseRefName": pick(raw, "/baseRefName"),
    "headRefName": pick(raw, "/headRefName"),
    "headRefOid": pick(raw, "/headRefOid"),
    "mergeable": pick(raw, "/mergeable"),
    "reviewDecision": decision,
    "statusCheckRollup": checks,
  })
}

/// The latest CI-reviewer comment that is a review, else the latest
/// CI-reviewer comment, with its parsed verdict; `absent` when there is none.
pub(crate) fn bot(comments: &[Value]) -> Value {
  let candidates: Vec<&Value> = comments
    .iter()
    .rev()
    .filter(|comment| {
      comment
        .get("author")
        .and_then(Value::as_str)
        .is_some_and(is_ci_reviewer)
    })
    .collect();
  let parse =
    |comment: &Value| verdict::parse(comment.get("body").and_then(Value::as_str).unwrap_or(""));
  let review = candidates.iter().find_map(|comment| {
    let parsed = parse(comment);
    (parsed.get("is_review_comment") == Some(&Value::Bool(true))).then_some((*comment, parsed))
  });
  let chosen = review.or_else(|| candidates.first().map(|comment| (*comment, parse(comment))));
  let Some((comment, verdict)) = chosen else {
    let mut verdict = verdict::parse("");
    if let Some(object) = verdict.as_object_mut() {
      object.insert("state".into(), Value::from("absent"));
    }
    return json!({"comment": null, "verdict": verdict});
  };
  json!({
    "comment": {
      "id": pick(comment, "/id"),
      "url": pick(comment, "/url"),
      "createdAt": pick(comment, "/createdAt"),
      "updatedAt": pick(comment, "/updatedAt"),
      "author": pick(comment, "/author"),
    },
    "verdict": verdict,
  })
}

#[cfg(test)]
#[path = "tests/normalize_test.rs"]
mod tests;
