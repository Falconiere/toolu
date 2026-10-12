//! The merge-gate verdict of one PR (`assess` in `merge-gate.ts`): merged or
//! closed, `rebase` when behind or conflicting, `fix` on failing checks,
//! unresolved threads or a draft, `wait` while checks run, else `merge`.

use serde_json::{Map, Value, json};

use crate::tracker::Gh;

const FAIL: [&str; 6] = [
  "FAILURE",
  "TIMED_OUT",
  "CANCELLED",
  "ACTION_REQUIRED",
  "STARTUP_FAILURE",
  "ERROR",
];
const PASS: [&str; 3] = ["SUCCESS", "NEUTRAL", "SKIPPED"];

const PR: &str = "query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){pullRequest(number:$n){
id state isDraft mergeable mergeStateStatus headRefOid headRefName baseRefName url autoMergeRequest{enabledAt}
reviewThreads(first:100){nodes{isResolved isOutdated} pageInfo{hasNextPage endCursor}}
commits(last:1){nodes{commit{statusCheckRollup{contexts(first:100){nodes{__typename
... on CheckRun{name status conclusion} ... on StatusContext{context state}}
pageInfo{hasNextPage endCursor}}}}}}}}}";

/// The review threads after cursor `$c`.
const THREADS: &str =
  "query($o:String!,$r:String!,$n:Int!,$c:String!){repository(owner:$o,name:$r){
pullRequest(number:$n){reviewThreads(first:100,after:$c){nodes{isResolved isOutdated}
pageInfo{hasNextPage endCursor}}}}}";

/// The head commit's check contexts after cursor `$c`.
const CONTEXTS: &str =
  "query($o:String!,$r:String!,$n:Int!,$c:String!){repository(owner:$o,name:$r){
pullRequest(number:$n){commits(last:1){nodes{commit{statusCheckRollup{contexts(first:100,after:$c){
nodes{__typename ... on CheckRun{name status conclusion} ... on StatusContext{context state}}
pageInfo{hasNextPage endCursor}}}}}}}}}";

/// Where each follow-up query's connection sits.
const THREADS_AT: &str = "/repository/pullRequest/reviewThreads";
const CONTEXTS_AT: &str =
  "/repository/pullRequest/commits/nodes/0/commit/statusCheckRollup/contexts";

/// Every node of a connection whose first page holds 100: follow its cursor
/// until GitHub reports no next page, so no check or thread past the first
/// page is missed. More than 50 pages fails rather than judge part of them.
fn all_nodes(gh: &Gh, pr: &Pr, first: &Value, query: &str, at: &str) -> Result<Vec<Value>, String> {
  let nodes_of = |connection: &Value| {
    connection
      .get("nodes")
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default()
  };
  let mut nodes = nodes_of(first);
  let mut page = first.get("pageInfo").cloned().unwrap_or(Value::Null);
  for _ in 0..50 {
    if page.get("hasNextPage") != Some(&Value::Bool(true)) {
      return Ok(nodes);
    }
    let cursor = page
      .get("endCursor")
      .and_then(Value::as_str)
      .ok_or_else(|| format!("{}#{}: a next page without a cursor", pr.slug(), pr.number))?;
    let vars = json!({"o": pr.owner, "r": pr.repo, "n": pr.number, "c": cursor});
    let data = gh.graphql(query, &vars, false)?;
    let connection = data
      .pointer(at)
      .cloned()
      .ok_or_else(|| format!("{}#{}: no page at {at}", pr.slug(), pr.number))?;
    nodes.extend(nodes_of(&connection));
    page = connection.get("pageInfo").cloned().unwrap_or(Value::Null);
  }
  Err(format!(
    "{}#{}: more than 50 pages at {at}",
    pr.slug(),
    pr.number
  ))
}

/// The PR a gate run acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pr {
  /// Owner.
  pub owner: String,
  /// Repository.
  pub repo: String,
  /// Number.
  pub number: u64,
}

impl Pr {
  /// `owner/repo`.
  pub(crate) fn slug(&self) -> String {
    format!("{}/{}", self.owner, self.repo)
  }
}

/// What a merge or auto-merge needs beyond the verdict.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Facts {
  /// The GraphQL node id.
  pub id: String,
  /// GitHub's `mergeStateStatus` (`BLOCKED` when protection refuses).
  pub merge_state: String,
}

/// `pass`, `fail` and `pending` check names of a status-check rollup.
pub(crate) fn check_buckets(rollup: &[Value]) -> Map<String, Value> {
  let (mut pass, mut fail, mut pending) = (Vec::new(), Vec::new(), Vec::new());
  for check in rollup {
    let field = |key: &str| {
      check
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    };
    let name = field("name")
      .or_else(|| field("context"))
      .unwrap_or("?")
      .to_owned();
    let bucket = if field("__typename") == Some("StatusContext") {
      match field("state") {
        Some("SUCCESS") => &mut pass,
        Some(state) if FAIL.contains(&state) => &mut fail,
        _ => &mut pending,
      }
    } else if field("status") != Some("COMPLETED") {
      &mut pending
    } else if field("conclusion").is_some_and(|conclusion| PASS.contains(&conclusion)) {
      &mut pass
    } else {
      &mut fail
    };
    bucket.push(name);
  }
  let mut out = Map::new();
  out.insert("pass".to_owned(), json!(pass));
  out.insert("fail".to_owned(), json!(fail));
  out.insert("pending".to_owned(), json!(pending));
  out
}

fn names(checks: &Map<String, Value>, bucket: &str) -> Vec<String> {
  checks
    .get(bucket)
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .map(str::to_owned)
    .collect()
}

/// The verdict and the reasons for it, from the PR's open-state facts.
pub(crate) fn verdict(facts: &Map<String, Value>, reasons: &mut Vec<String>) -> &'static str {
  let behind = facts.get("behind_by").and_then(Value::as_u64).unwrap_or(0);
  let mergeable = facts.get("mergeable").and_then(Value::as_str).unwrap_or("");
  let checks = facts
    .get("checks")
    .and_then(Value::as_object)
    .cloned()
    .unwrap_or_default();
  let unresolved = facts
    .get("unresolved_threads")
    .and_then(Value::as_u64)
    .unwrap_or(0);
  let draft = facts.get("draft").and_then(Value::as_bool).unwrap_or(false);
  if mergeable == "CONFLICTING" || behind > 0 {
    reasons.push(if mergeable == "CONFLICTING" {
      "conflicting with base".to_owned()
    } else {
      format!("behind base by {behind}")
    });
    return "rebase";
  }
  let failing = names(&checks, "fail");
  if !failing.is_empty() || unresolved > 0 || draft {
    if !failing.is_empty() {
      reasons.push(format!("failing: {}", failing.join(", ")));
    }
    if unresolved > 0 {
      reasons.push(format!("{unresolved} unresolved review threads"));
    }
    if draft {
      reasons.push("draft".to_owned());
    }
    return "fix";
  }
  let pending = names(&checks, "pending");
  if !pending.is_empty() || mergeable == "UNKNOWN" {
    let what = if pending.is_empty() {
      "mergeability not computed".to_owned()
    } else {
      pending.join(", ")
    };
    reasons.push(format!("pending: {what}"));
    return "wait";
  }
  "merge"
}

/// The checks of an open PR, with a placeholder while workflows exist but
/// report nothing yet.
fn checks_of(gh: &Gh, pr: &Pr, node: &Value) -> Result<Map<String, Value>, String> {
  let rollup = match node.pointer("/commits/nodes/0/commit/statusCheckRollup/contexts") {
    Some(first) => Some(all_nodes(gh, pr, first, CONTEXTS, CONTEXTS_AT)?),
    None => None,
  };
  let mut checks = check_buckets(rollup.as_deref().unwrap_or(&[]));
  if rollup.is_none() {
    let workflows: Value = gh.get(&format!("/repos/{}/actions/workflows", pr.slug()))?;
    if workflows
      .get("total_count")
      .and_then(Value::as_u64)
      .unwrap_or(0)
      > 0
      && let Some(Value::Array(pending)) = checks.get_mut("pending")
    {
      pending.push(json!("(no checks reported yet)"));
    }
  }
  Ok(checks)
}

/// The open-state facts of a PR: how far behind, mergeability, checks,
/// unresolved threads, draft and base.
fn open_facts(
  gh: &Gh,
  pr: &Pr,
  node: &Value,
  result: &mut Map<String, Value>,
) -> Result<(), String> {
  let field = |key: &str| {
    node
      .get(key)
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned()
  };
  let compare: Value = gh.get(&format!(
    "/repos/{}/compare/{}...{}",
    pr.slug(),
    field("baseRefName"),
    field("headRefOid")
  ))?;
  let threads = match node.get("reviewThreads") {
    Some(first) => all_nodes(gh, pr, first, THREADS, THREADS_AT)?,
    None => Vec::new(),
  };
  let unresolved = threads
    .iter()
    .filter(|thread| thread.get("isResolved") != Some(&Value::Bool(true)))
    .count();
  result.insert(
    "behind_by".to_owned(),
    json!(
      compare
        .get("behind_by")
        .and_then(Value::as_u64)
        .unwrap_or(0)
    ),
  );
  result.insert("mergeable".to_owned(), json!(field("mergeable")));
  result.insert("checks".to_owned(), Value::Object(checks_of(gh, pr, node)?));
  result.insert("unresolved_threads".to_owned(), json!(unresolved));
  result.insert(
    "draft".to_owned(),
    json!(node.get("isDraft") == Some(&Value::Bool(true))),
  );
  result.insert("base".to_owned(), json!(field("baseRefName")));
  Ok(())
}

/// Assess `pr`: the gate document (without settlement) and the merge facts.
///
/// # Errors
/// A GitHub call fails.
pub(crate) fn assess(gh: &Gh, pr: &Pr) -> Result<(Map<String, Value>, Facts), String> {
  let vars = json!({"o": pr.owner, "r": pr.repo, "n": pr.number});
  let data = gh.graphql(PR, &vars, false)?;
  let node = data
    .pointer("/repository/pullRequest")
    .cloned()
    .ok_or_else(|| format!("no pull request {}#{}", pr.slug(), pr.number))?;
  let field = |key: &str| {
    node
      .get(key)
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned()
  };
  let mut result = Map::new();
  result.insert(
    "pr".to_owned(),
    json!(format!("{}#{}", pr.slug(), pr.number)),
  );
  result.insert("url".to_owned(), json!(field("url")));
  result.insert("head".to_owned(), json!(field("headRefOid")));
  let armed = node
    .get("autoMergeRequest")
    .is_some_and(|auto| !auto.is_null());
  result.insert("auto_merge_armed".to_owned(), json!(armed));
  let facts = Facts {
    id: field("id"),
    merge_state: field("mergeStateStatus"),
  };
  let mut reasons = Vec::new();
  let verdict = if field("state") == "OPEN" {
    open_facts(gh, pr, &node, &mut result)?;
    verdict(&result, &mut reasons).to_owned()
  } else {
    field("state").to_lowercase()
  };
  result.insert("reasons".to_owned(), json!(reasons));
  result.insert("verdict".to_owned(), json!(verdict));
  Ok((result, facts))
}

#[cfg(test)]
#[path = "tests/assess_test.rs"]
mod tests;
