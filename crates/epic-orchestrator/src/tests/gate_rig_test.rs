//! A GitHub PR on the HTTPS fixture for gate tests: the GraphQL facts, the
//! compare and workflow endpoints, and the repository's merge methods.

use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use crate::gate::{MakeTracker, Settle};
use crate::tracker::github::{ClosePolls, GitHubTracker};
use crate::tracker::{Gh, Tracker, TrackerKind};

/// The PR facts a test serves.
pub(crate) struct PrState {
  pub state: &'static str,
  pub mergeable: &'static str,
  pub merge_state: &'static str,
  pub draft: bool,
  pub behind: u64,
  pub rollup: Value,
  pub unresolved: usize,
  pub armed: bool,
}

impl Default for PrState {
  fn default() -> Self {
    let green = json!([{"__typename": "CheckRun", "name": "test", "status": "COMPLETED", "conclusion": "SUCCESS"}]);
    Self {
      state: "OPEN",
      mergeable: "MERGEABLE",
      merge_state: "CLEAN",
      draft: false,
      behind: 0,
      rollup: green,
      unresolved: 0,
      armed: false,
    }
  }
}

/// Serve `pr` as `o/r#7` at head `abc123`.
pub(crate) fn serve_pr(fixture: &Fixture, pr: &PrState) {
  let threads: Vec<Value> = (0..pr.unresolved)
    .map(|_| json!({"isResolved": false, "isOutdated": false}))
    .collect();
  let rollup = if pr.rollup.is_null() {
    Value::Null
  } else {
    json!({"contexts": {"nodes": pr.rollup}})
  };
  let node = json!({
    "id": "PR_7", "state": pr.state, "isDraft": pr.draft, "mergeable": pr.mergeable,
    "mergeStateStatus": pr.merge_state, "headRefOid": "abc123", "headRefName": "feat/7-x",
    "baseRefName": "main", "url": "https://github.com/o/r/pull/7",
    "autoMergeRequest": if pr.armed { json!({"enabledAt": "2026-10-11T00:00:00Z"}) } else { Value::Null },
    "reviewThreads": {"nodes": threads},
    "commits": {"nodes": [{"commit": {"statusCheckRollup": rollup}}]},
  });
  let reply = |body: Value| Reply::new(200, body.to_string());
  fixture
    .route(
      "/graphql",
      reply(json!({"data": {"repository": {"pullRequest": node}}})),
    )
    .expect("graphql");
  fixture
    .route(
      "/repos/o/r/compare/main...abc123",
      reply(json!({"behind_by": pr.behind})),
    )
    .expect("compare");
  fixture
    .route(
      "/repos/o/r/actions/workflows",
      reply(json!({"total_count": 1})),
    )
    .expect("workflows");
  fixture
    .route(
      "/repos/o/r",
      reply(json!({"allow_squash_merge": true, "allow_merge_commit": true})),
    )
    .expect("repo");
}

/// GitHub trackers on the fixture that close without waiting.
pub(crate) fn trackers(fixture: &Fixture) -> Box<MakeTracker<'_>> {
  Box::new(move |kind: TrackerKind, reference: &str, _repo: &str| {
    if kind != TrackerKind::Github {
      return Err(format!("no {} tracker in this test", kind.name()));
    }
    let polls = ClosePolls {
      count: 1,
      wait: std::time::Duration::ZERO,
    };
    let tracker: Box<dyn Tracker + Sync> = Box::new(GitHubTracker::with(
      reference,
      crate::support::gh(fixture),
      polls,
    )?);
    Ok(tracker)
  })
}

/// The settlement context of a test.
pub(crate) fn settle<'a>(env: &'a Env, gh: &'a Gh, make: &'a MakeTracker<'a>) -> Settle<'a> {
  Settle { env, gh, make }
}
