//! Test support: recorded GitHub replies for Falconiere/toolu#115 served over
//! loopback TLS, the way GitHub answers the Rust collector.

use serde_json::{Value, json};
use toolu_github::{Client, Config};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use crate::fixture;

/// The recorded pull request's REST root.
pub(crate) const ROOT: &str = "/repos/Falconiere/toolu";

/// A one-attempt client configuration for `fixture`'s API.
pub(crate) fn config(fixture: &Fixture) -> Config {
  let mut config = Config::scheduled();
  config.api_url = fixture.url("");
  config.http.test_root_ca_der = Some(fixture.root_ca_der().to_vec());
  config
}

/// An environment with a token that reaches `fixture` through its proxy.
pub(crate) fn env(fixture: &Fixture) -> Env {
  Env::from_pairs([
    ("GH_TOKEN", "recorded-token"),
    ("HTTPS_PROXY", &fixture.proxy_url()),
  ])
}

/// A one-attempt client of `fixture`'s API.
pub(crate) fn client(fixture: &Fixture) -> Client {
  Client::new(config(fixture), &env(fixture)).unwrap()
}

/// The recorded snapshot the replies reproduce.
pub(crate) fn snapshot() -> Value {
  fixture::json("snapshots/toolu-115.json")
}

/// The PR query's reply, rebuilt from the recorded `gh pr view` document.
pub(crate) fn pr_page() -> Value {
  let mut pr = snapshot()["pr"].clone();
  let checks: Vec<Value> = pr["statusCheckRollup"]
    .as_array()
    .unwrap()
    .iter()
    .map(|check| {
      let mut node = check.clone();
      let workflow = node
        .as_object_mut()
        .unwrap()
        .remove("workflowName")
        .unwrap();
      node["checkSuite"] = json!({"workflowRun": {"workflow": {"name": workflow}}});
      node
    })
    .collect();
  let object = pr.as_object_mut().unwrap();
  object.remove("statusCheckRollup");
  let author = object.remove("author").unwrap();
  object.insert("author".into(), json!({"login": author}));
  object.insert(
    "commits".into(),
    json!({"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": checks}}}}]}),
  );
  json!({"data": {"rateLimit": {"cost": 1}, "repository": {"pullRequest": pr}}})
}

/// The recorded review-thread pages (page size 2), each costing one point.
pub(crate) fn thread_pages() -> Vec<Value> {
  let mut pages = fixture::json("github/pages-115-threads-p2.json")
    .as_array()
    .unwrap()
    .clone();
  for page in &mut pages {
    page["data"]["rateLimit"] = json!({"cost": 1});
  }
  pages
}

/// `pages` as replies, the last one carrying the remaining points.
pub(crate) fn replies(pages: &[Value]) -> Vec<Reply> {
  let last = pages.len().saturating_sub(1);
  pages
    .iter()
    .enumerate()
    .map(|(index, page)| {
      let reply = Reply::new(200, page.to_string());
      if index == last {
        reply
          .header("X-RateLimit-Remaining", "4990")
          .header("X-RateLimit-Reset", "1760000000")
      } else {
        reply
      }
    })
    .collect()
}

/// Serve the recorded REST pages at `path`, linked by `Link: rel="next"`.
pub(crate) fn rest_pages(fixture: &Fixture, path: &str, pages: &[Value]) {
  for (index, page) in pages.iter().enumerate() {
    let at = |n: usize| {
      if n == 0 {
        path.to_owned()
      } else {
        format!("{path}&page={}", n + 1)
      }
    };
    let mut reply = Reply::new(200, page.to_string());
    if index + 1 < pages.len() {
      reply = reply.header(
        "Link",
        &format!("<{}>; rel=\"next\"", fixture.url(&at(index + 1))),
      );
    }
    fixture.route(&at(index), reply).unwrap();
  }
}

/// Serve PR 115: the head SHAs in order, the GraphQL replies in order, and the
/// recorded comment and review pages.
pub(crate) fn serve(fixture: &Fixture, heads: &[&str], graphql: Vec<Reply>) {
  let heads = heads
    .iter()
    .map(|sha| Reply::new(200, json!({"head": {"sha": sha}}).to_string()))
    .collect();
  fixture
    .sequence(&format!("{ROOT}/pulls/115"), heads)
    .unwrap();
  fixture.sequence("/graphql", graphql).unwrap();
  let comments = fixture::json("github/pages-115-comments-p2.json");
  let reviews = fixture::json("github/pages-115-reviews-p2.json");
  rest_pages(
    fixture,
    &format!("{ROOT}/issues/115/comments?per_page=2"),
    comments.as_array().unwrap(),
  );
  rest_pages(
    fixture,
    &format!("{ROOT}/pulls/115/reviews?per_page=2"),
    reviews.as_array().unwrap(),
  );
}

/// The head SHA of the open pull request [`serve_open`] serves.
pub(crate) const OPEN_SHA: &str = "0123456789abcdef0123456789abcdef01234567";

/// A GraphQL check-run node.
pub(crate) fn check_run(name: &str, status: &str, conclusion: Option<&str>) -> Value {
  json!({"__typename": "CheckRun", "name": name, "status": status, "conclusion": conclusion,
    "detailsUrl": format!("https://github.com/Falconiere/toolu/actions/{name}"),
    "startedAt": "2026-10-11T12:00:00Z", "completedAt": null,
    "checkSuite": {"workflowRun": {"workflow": {"name": "tests"}}}})
}

/// Serve an open PR 115 at page size 100 with no review threads, `checks` as
/// its head commit's checks and `verdict` as the review bot's comment, last
/// updated at `updated`. One GraphQL reply answers both the PR and the
/// threads query.
pub(crate) fn serve_open(fixture: &Fixture, checks: &[Value], verdict: &str, updated: &str) {
  let pr = json!({
    "number": 115, "title": "feat: open", "url": "https://github.com/Falconiere/toolu/pull/115",
    "author": {"login": "Falconiere"}, "state": "OPEN", "baseRefName": "main",
    "headRefName": "feat/open", "headRefOid": OPEN_SHA, "mergeable": "MERGEABLE",
    "reviewDecision": null,
    "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": checks}}}}]},
    "reviewThreads": {"pageInfo": {"hasNextPage": false, "endCursor": null}, "nodes": []},
  });
  let page = json!({"data": {"rateLimit": {"cost": 1}, "repository": {"pullRequest": pr}}});
  fixture
    .route("/graphql", Reply::new(200, page.to_string()))
    .unwrap();
  let head = json!({"head": {"sha": OPEN_SHA}}).to_string();
  fixture
    .route(&format!("{ROOT}/pulls/115"), Reply::new(200, head))
    .unwrap();
  let comment = json!([{
    "id": 1, "body": verdict, "user": {"login": "github-actions[bot]", "type": "Bot"},
    "created_at": "2026-10-11T12:00:00Z", "updated_at": updated,
    "html_url": "https://github.com/Falconiere/toolu/pull/115#issuecomment-1",
  }]);
  let comments = format!("{ROOT}/issues/115/comments?per_page=100");
  fixture
    .route(&comments, Reply::new(200, comment.to_string()))
    .unwrap();
  let reviews = format!("{ROOT}/pulls/115/reviews?per_page=100");
  fixture.route(&reviews, Reply::new(200, "[]")).unwrap();
}

/// The GraphQL replies of one unchanged collection.
pub(crate) fn round() -> Vec<Value> {
  let mut pages = vec![pr_page()];
  pages.extend(thread_pages());
  pages
}

/// Serve one unchanged collection of PR 115 at the recorded head.
pub(crate) fn serve_round(fixture: &Fixture) {
  let sha = snapshot()["head"]["sha"].as_str().unwrap().to_owned();
  serve(fixture, &[&sha, &sha], replies(&round()));
}

#[cfg(test)]
#[path = "tests/recorded_test.rs"]
mod tests;
