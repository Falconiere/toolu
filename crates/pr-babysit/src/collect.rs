//! `collect`: one pull request read over toolu-github into a version-1
//! snapshot — the PR and its head commit's checks, review threads with their
//! overflowing comments, issue comments and reviews — with the head SHA read
//! before and after so a push mid-read recollects once, then fails.

mod normalize;
mod read;

use serde_json::{Value, json};
use toolu_engine::babysit::GraphQlUsage;
use toolu_github::Client;

use crate::error::{Failure, Result};
use crate::json::array;
use read::Reader;

const PR_QUERY: &str = "query($owner:String!,$repo:String!,$number:Int!){
  rateLimit{cost}
  repository(owner:$owner,name:$repo){ pullRequest(number:$number){
    number title url author{login} state baseRefName headRefName headRefOid mergeable
    reviewDecision
    commits(last:1){ nodes{ commit{ statusCheckRollup{ contexts(first:100){ nodes{ __typename
      ... on CheckRun{ name status conclusion detailsUrl startedAt completedAt
        checkSuite{ workflowRun{ workflow{ name } } } }
      ... on StatusContext{ context state targetUrl createdAt } } } } } } } } } }";
const THREADS_QUERY: &str =
  "query($owner:String!,$repo:String!,$number:Int!,$pageSize:Int!,$endCursor:String){
  rateLimit{cost}
  repository(owner:$owner,name:$repo){ pullRequest(number:$number){
    reviewThreads(first:$pageSize,after:$endCursor){
      pageInfo{hasNextPage endCursor}
      nodes{ id isResolved isOutdated path line
        comments(first:$pageSize){ pageInfo{hasNextPage endCursor}
          nodes{ id databaseId body author{login __typename} createdAt url } } } } } } }";
const COMMENTS_QUERY: &str = "query($id:ID!,$pageSize:Int!,$endCursor:String){
  rateLimit{cost}
  node(id:$id){ ... on PullRequestReviewThread {
    comments(first:$pageSize,after:$endCursor){ pageInfo{hasNextPage endCursor}
      nodes{ id databaseId body author{login __typename} createdAt url } } } } }";
const THREADS: &str = "/repository/pullRequest/reviewThreads";

/// The pull request to read.
pub(crate) struct Request<'a> {
  /// `owner/repo`.
  pub(crate) repo: &'a str,
  /// The pull request number.
  pub(crate) number: u64,
  /// Items per page.
  pub(crate) page_size: u64,
  /// `collectedAt`.
  pub(crate) now: &'a str,
}

/// A snapshot and what its GraphQL reads cost.
pub(crate) struct Collected {
  /// The version-1 snapshot.
  pub(crate) snapshot: Value,
  /// The GraphQL points spent and the latest limits.
  pub(crate) graphql: GraphQlUsage,
}

struct Parts {
  pr: Value,
  threads: Vec<Value>,
  comments: Vec<Value>,
  reviews: Vec<Value>,
  pages: Value,
}

/// Read `request`'s pull request.
///
/// # Errors
/// `api_error` or `invalid_json` for a failed read, `gh_unavailable` without a
/// token, `head_moved` when the head moved during two collections.
pub(crate) fn collect(client: &Client, request: &Request<'_>) -> Result<Collected> {
  let mut reader = Reader::new(client);
  let mut recollected = false;
  for _ in 0..2 {
    let before = reader.head(request.repo, request.number)?;
    let parts = fan_out(&mut reader, request)?;
    let after = reader.head(request.repo, request.number)?;
    if before != after {
      recollected = true;
      continue;
    }
    let snapshot = json!({
      "version": 1,
      "collectedAt": request.now,
      "repo": request.repo,
      "number": request.number,
      "head": {"sha": after, "verifiedAfterFanout": true, "recollected": recollected},
      "pageSize": request.page_size,
      "pr": parts.pr,
      "threads": parts.threads,
      "bot": normalize::bot(&parts.comments),
      "comments": parts.comments,
      "reviews": parts.reviews,
      "pages": parts.pages,
    });
    return Ok(Collected {
      snapshot,
      graphql: reader.usage,
    });
  }
  Err(Failure::new("head_moved", "PR head moved twice during collection").with("source", "head"))
}

fn fan_out(reader: &mut Reader<'_>, request: &Request<'_>) -> Result<Parts> {
  let (owner, name) = request.repo.split_once('/').unwrap_or((request.repo, ""));
  let base = json!({"owner": owner, "repo": name, "number": request.number});
  let pr = reader.graphql("pr", PR_QUERY, &base)?;
  let raw = pr
    .pointer("/repository/pullRequest")
    .filter(|raw| raw.is_object());
  let raw =
    raw.ok_or_else(|| Failure::new("api_error", "read 'pr' failed: no such pull request"))?;
  let checks: Vec<Value> =
    array(raw.pointer("/commits/nodes/0/commit/statusCheckRollup/contexts/nodes"))
      .iter()
      .map(normalize::check)
      .collect();
  let mut variables = base.clone();
  if let Some(object) = variables.as_object_mut() {
    object.insert("pageSize".into(), json!(request.page_size));
  }
  let thread_pages = reader.graphql_pages("threads", THREADS_QUERY, &variables, THREADS)?;
  let rest = |kind: &str| format!("/repos/{}/{kind}/{}", request.repo, request.number);
  let per_page = format!("?per_page={}", request.page_size);
  let comment_pages = reader.rest_pages("comments", &(rest("issues") + "/comments" + &per_page))?;
  let review_pages = reader.rest_pages("reviews", &(rest("pulls") + "/reviews" + &per_page))?;
  let (threads, overflow_pages) = threads(reader, &thread_pages, request.page_size)?;
  let flat = |pages: &[Value], shape: fn(&Value) -> Value| -> Vec<Value> {
    pages
      .iter()
      .flat_map(|page| array(Some(page)).iter().map(shape))
      .collect()
  };
  Ok(Parts {
    pr: normalize::pr(raw, &checks),
    threads,
    comments: flat(&comment_pages, normalize::comment),
    reviews: flat(&review_pages, normalize::review),
    pages: json!({
      "threads": thread_pages.len(),
      "threadComments": overflow_pages,
      "comments": comment_pages.len(),
      "reviews": review_pages.len(),
    }),
  })
}

fn threads(
  reader: &mut Reader<'_>,
  pages: &[Value],
  page_size: u64,
) -> Result<(Vec<Value>, usize)> {
  let mut threads = Vec::new();
  let mut overflow_pages = 0;
  for raw in pages
    .iter()
    .flat_map(|page| array(page.pointer(&format!("{THREADS}/nodes"))))
  {
    let (mut thread, cursor) = normalize::thread(raw);
    if let Some(cursor) = cursor {
      let id = thread.get("id").cloned();
      let variables = json!({"id": id, "pageSize": page_size, "endCursor": cursor});
      let extra = reader.graphql_pages(
        "threadComments",
        COMMENTS_QUERY,
        &variables,
        "/node/comments",
      )?;
      overflow_pages += extra.len();
      if let Some(comments) = thread.get_mut("comments").and_then(Value::as_array_mut) {
        comments.extend(
          extra
            .iter()
            .flat_map(|page| array(page.pointer("/node/comments/nodes")))
            .map(normalize::thread_comment),
        );
      }
    }
    threads.push(thread);
  }
  Ok((threads, overflow_pages))
}

#[cfg(test)]
#[path = "tests/collect_test.rs"]
mod tests;
