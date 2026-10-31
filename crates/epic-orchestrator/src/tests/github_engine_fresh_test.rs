//! #761: a quiet scheduled check reuses the last keep-going collection for at
//! most `REFRESH_MS`, and anything that may have changed collects again.

use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};
use toolu_pr_babysit::GitHubTick;

use super::babysit_tests::{SLOT_MS, babysit, bot_comment, ci_thread, graphql, journal, prompts};
use super::fixture::{engine, rest_bodies};
use crate::server::Engine;

/// CI still running and the review bot still working: keep going, no prompt.
pub(super) fn pending() -> (String, Value) {
  let mut body: Value = serde_json::from_str(&graphql("SUCCESS", &[])).expect("GraphQL");
  let check = &mut body["data"]["repository"]["pullRequest"]["commits"]["nodes"][0]["commit"]["statusCheckRollup"]
    ["contexts"]["nodes"][0];
  check["status"] = json!("IN_PROGRESS");
  check["conclusion"] = Value::Null;
  let comments = bot_comment("in-progress.txt", "2026-10-11T10:05:00Z");
  (body.to_string(), comments)
}

/// Answer every REST path with `comments` as the issue comments: `200` with
/// an `ETag` when `fresh`, else `304` to the probe and `200` to a collection.
pub(super) fn rest(fixture: &Fixture, comments: &Value, fresh: bool) {
  for (path, body) in rest_bodies(comments.to_string()) {
    let full = Reply::new(200, body).header("ETag", &format!("\"{}\"", path.len()));
    let replies = if fresh {
      vec![full]
    } else {
      vec![Reply::new(304, ""), full]
    };
    fixture.sequence(&path, replies).expect("REST route");
  }
}

pub(super) fn graphql_requests(fixture: &Fixture) -> usize {
  let requests = fixture.requests().expect("requests");
  requests.iter().filter(|r| r.path == "/graphql").count()
}

/// Run scheduled slot `slot` with quiet REST replies; whether it collected.
pub(super) fn quiet_slot(
  engine: &mut Engine,
  fixture: &Fixture,
  tick: &GitHubTick,
  comments: &Value,
  slot: u64,
) -> bool {
  rest(fixture, comments, false);
  let before = graphql_requests(fixture);
  engine.world.now_ms = slot * SLOT_MS;
  engine.github_tick(tick);
  graphql_requests(fixture) > before
}

/// An engine whose first (immediate) check collected `pending()`.
pub(super) fn settled(fixture: &Fixture) -> (tempfile::TempDir, Engine, GitHubTick, Value) {
  let (body, comments) = pending();
  rest(fixture, &comments, true);
  fixture
    .route("/graphql", Reply::new(200, body))
    .expect("GraphQL route");
  let (temp, mut engine, _) = engine(fixture);
  let tick = babysit(fixture);
  engine.github_tick(&tick);
  assert!(graphql_requests(fixture) > 0, "the first check collects");
  assert!(engine.world.watches["pr:o/r#1"].babysit.is_some());
  (temp, engine, tick, comments)
}

fn skipped_rows(engine: &Engine) -> Vec<Value> {
  journal(engine)
    .iter()
    .filter(|row| row.name == "github-check")
    .filter_map(|row| serde_json::from_str::<Value>(&row.note).ok())
    .filter(|note| note["babysit"] == "skipped")
    .collect()
}

#[test]
fn quiet_slots_skip_collection_for_at_most_fifteen_minutes() {
  let fixture = Fixture::start().expect("fixture");
  let (_temp, mut engine, tick, comments) = settled(&fixture);
  for slot in 1..5 {
    assert!(
      !quiet_slot(&mut engine, &fixture, &tick, &comments, slot),
      "slot {slot} skips"
    );
  }
  assert!(quiet_slot(&mut engine, &fixture, &tick, &comments, 5));
  let skipped = skipped_rows(&engine);
  assert_eq!(skipped.len(), 4);
  assert!(skipped.iter().all(|note| note["babysitCollectedAt"] == 0
    && note["graphqlTick"] == 0
    && note.get("next").is_none()));
  assert_eq!(engine.world.issues["one"].phase, "babysit");
  assert!(
    !quiet_slot(&mut engine, &fixture, &tick, &comments, 6),
    "the new collection is reused"
  );
}

#[test]
fn a_thread_resolved_only_in_graphql_is_seen_within_the_bound_and_never_merged_early() {
  let fixture = Fixture::start().expect("fixture");
  let approved = bot_comment("pr122-verdict-approved.txt", "2026-10-11T10:05:00Z");
  rest(&fixture, &approved, true);
  fixture
    .route(
      "/graphql",
      Reply::new(200, graphql("SUCCESS", &[ci_thread(false)])),
    )
    .expect("GraphQL route");
  let (_temp, mut engine, _) = engine(&fixture);
  let tick = babysit(&fixture);
  engine.github_tick(&tick);
  assert_eq!(prompts(&engine).len(), 1, "the open thread prompts");
  assert!(
    quiet_slot(&mut engine, &fixture, &tick, &approved, 1),
    "a prompt is not reused"
  );
  fixture
    .route(
      "/graphql",
      Reply::new(200, graphql("SUCCESS", &[ci_thread(true)])),
    )
    .expect("resolved in the UI");
  for slot in 2..6 {
    assert!(!quiet_slot(&mut engine, &fixture, &tick, &approved, slot));
    assert_eq!(engine.world.issues["one"].phase, "babysit", "slot {slot}");
  }
  assert!(quiet_slot(&mut engine, &fixture, &tick, &approved, 6));
  assert_eq!(engine.world.issues["one"].phase, "ready");
  assert_eq!(prompts(&engine).len(), 1);
}

#[test]
fn each_forcing_condition_alone_collects() {
  type Force = fn(&mut Engine, &Fixture);
  let cases: [(&str, Force); 5] = [
    ("immediate", |engine, _| {
      let watch = engine.world.watches.get_mut("pr:o/r#1").expect("watch");
      watch.request_immediate();
    }),
    ("fresh REST", |_, fixture| {
      let path = "/repos/o/r/pulls/1/reviews?per_page=100";
      let reply = Reply::new(200, "[]").header("ETag", "\"new\"");
      fixture.route(path, reply).expect("fresh reviews");
    }),
    ("ready", |engine, _| {
      engine.world.issues.get_mut("one").expect("issue").phase = "ready".into();
    }),
    ("edited state", |engine, _| {
      let state = engine.paths.root.join("babysit/o__r-1.json");
      let mut text = std::fs::read_to_string(&state).expect("state");
      text.push('\n');
      std::fs::write(&state, text).expect("edit state");
    }),
    ("absent state", |engine, _| {
      let state = engine.paths.root.join("babysit/o__r-1.json");
      std::fs::remove_file(state).expect("remove state");
    }),
  ];
  for (name, force) in cases {
    let fixture = Fixture::start().expect("fixture");
    let (_temp, mut engine, tick, comments) = settled(&fixture);
    rest(&fixture, &comments, false);
    force(&mut engine, &fixture);
    let before = graphql_requests(&fixture);
    engine.world.now_ms = SLOT_MS;
    engine.github_tick(&tick);
    assert!(graphql_requests(&fixture) > before, "{name} collects");
  }
}

#[test]
fn a_rate_limited_tick_clears_the_memo_so_the_slot_after_the_hold_collects() {
  let fixture = Fixture::start().expect("fixture");
  let (_temp, mut engine, tick, comments) = settled(&fixture);
  let (body, _) = pending();
  let limited = Reply::new(403, r#"{"message":"API rate limit exceeded"}"#)
    .header("Retry-After", "60")
    .header("X-RateLimit-Remaining", "0");
  fixture
    .sequence("/graphql", vec![limited, Reply::new(200, body)])
    .expect("GraphQL route");
  engine
    .world
    .watches
    .get_mut("pr:o/r#1")
    .expect("watch")
    .request_immediate();
  rest(&fixture, &comments, false);
  engine.world.now_ms = 1;
  engine.github_tick(&tick);
  assert!(engine.world.github_hold_until_ms > 1, "the hold is set");
  assert!(engine.world.watches["pr:o/r#1"].babysit.is_none());
  assert!(quiet_slot(&mut engine, &fixture, &tick, &comments, 1));
}

#[test]
fn a_keep_going_collection_made_while_ready_is_not_reused_back_in_babysit() {
  let fixture = Fixture::start().expect("fixture");
  let (_temp, mut engine, tick, comments) = settled(&fixture);
  engine.world.issues.get_mut("one").expect("issue").phase = "ready".into();
  assert!(
    quiet_slot(&mut engine, &fixture, &tick, &comments, 1),
    "ready collects"
  );
  assert!(engine.world.watches["pr:o/r#1"].babysit.is_none());
  engine.world.issues.get_mut("one").expect("issue").phase = "babysit".into();
  assert!(
    quiet_slot(&mut engine, &fixture, &tick, &comments, 2),
    "back in babysit"
  );
}

#[test]
fn a_stint_outside_babysit_drops_the_memo() {
  let fixture = Fixture::start().expect("fixture");
  let (_temp, mut engine, tick, comments) = settled(&fixture);
  engine.world.issues.get_mut("one").expect("issue").phase = "pr-open".into();
  assert!(
    !quiet_slot(&mut engine, &fixture, &tick, &comments, 1),
    "pr-open never collects"
  );
  assert!(engine.world.watches["pr:o/r#1"].babysit.is_none());
  engine.world.issues.get_mut("one").expect("issue").phase = "babysit".into();
  assert!(
    quiet_slot(&mut engine, &fixture, &tick, &comments, 2),
    "back in babysit"
  );
}

/// An engine whose first check collected `body` with `comments`.
fn first_check(
  fixture: &Fixture,
  body: String,
  comments: &Value,
) -> (tempfile::TempDir, Engine, GitHubTick) {
  rest(fixture, comments, true);
  fixture
    .route("/graphql", Reply::new(200, body))
    .expect("GraphQL route");
  let (temp, mut engine, _) = engine(fixture);
  let tick = babysit(fixture);
  engine.github_tick(&tick);
  (temp, engine, tick)
}

#[test]
fn a_keep_going_while_mergeability_is_unknown_is_not_reused() {
  let fixture = Fixture::start().expect("fixture");
  let (body, comments) = pending();
  let mut unknown: Value = serde_json::from_str(&body).expect("GraphQL");
  unknown["data"]["repository"]["pullRequest"]["mergeable"] = json!("UNKNOWN");
  let (_temp, mut engine, tick) = first_check(&fixture, unknown.to_string(), &comments);
  assert!(engine.world.watches["pr:o/r#1"].babysit.is_none());
  assert!(quiet_slot(&mut engine, &fixture, &tick, &comments, 1));
}

#[test]
fn a_full_last_page_of_comments_is_never_trusted_to_be_complete() {
  let fixture = Fixture::start().expect("fixture");
  let (body, _) = pending();
  let comment = json!({"id": 1, "body": "noted", "user": {"login": "worker", "type": "User"},
    "created_at": "2026-10-11T10:00:00Z", "updated_at": "2026-10-11T10:00:00Z",
    "html_url": "https://github.com/o/r/pull/1#issuecomment-1"});
  let comments = Value::Array(vec![comment; 100]);
  let (_temp, mut engine, tick) = first_check(&fixture, body, &comments);
  assert!(!engine.world.watches["pr:o/r#1"].full_tails.is_empty());
  assert!(quiet_slot(&mut engine, &fixture, &tick, &comments, 1));
}

#[test]
fn a_base_move_drops_the_memos_of_that_repositorys_pull_requests() {
  let fixture = Fixture::start().expect("fixture");
  let (_temp, mut engine, tick, comments) = settled(&fixture);
  let moved = crate::github_probe::Change::BaseMoved {
    repo: "o/r".into(),
    branch: "main".into(),
  };
  super::note_change(&mut engine.world, moved);
  assert!(engine.world.watches["pr:o/r#1"].babysit.is_none());
  assert!(quiet_slot(&mut engine, &fixture, &tick, &comments, 1));
}
