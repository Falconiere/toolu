use serde_json::Value;
use toolu_http_test_support::{Fixture, Reply};

use super::{cause_name, rearm};
use crate::disk::read_value;
use crate::journal;
use crate::model::{Issue, Step, World};
use crate::status;
use crate::watch::{self, Cause, Kind, Watch};

use super::fixture::{engine, graphql_body, pr_reply, rest_routes};

#[test]
fn github_watch_full_tick_promotes_green_pr_without_worker_prompt() {
  let fixture = Fixture::start().expect("fixture");
  rest_routes(&fixture);
  fixture
    .sequence(
      "/graphql",
      vec![
        Reply::new(200, graphql_body(false)).header("X-RateLimit-Remaining", "4999"),
        Reply::new(200, graphql_body(true)).header("X-RateLimit-Remaining", "4998"),
      ],
    )
    .expect("GraphQL route");
  let (_temp, mut engine, tick) = engine(&fixture);
  engine.github_tick(&tick);
  assert_eq!(engine.world.issues["one"].phase, "babysit");
  engine.world.now_ms = 180_000;
  engine.github_tick(&tick);
  assert_eq!(engine.world.issues["one"].phase, "ready");
  assert_eq!(engine.world.graphql_points, 2);
  assert!(
    engine
      .world
      .outbox
      .iter()
      .any(|step| matches!(step, Step::Journal(row) if row.name == "github-babysit-ready"))
  );
  assert!(
    !engine
      .world
      .outbox
      .iter()
      .any(|step| matches!(step, Step::Journal(row) if row.name == "prompt-intent"))
  );
  let view = status::document(&engine.world, true, None);
  assert_eq!(view["github"]["graphql"]["points"], 2);
  assert_eq!(view["issues"][0]["github"]["lastCheckAt"], 180_000);
  assert_eq!(view["issues"][0]["github"]["nextCheckAt"], 360_000);
  engine.flush().expect("flush");
  let rows = journal::tail(&engine.paths.journal_dir(), engine.now).expect("journal");
  assert!(rows.iter().any(|row| row.name == "github-babysit-ready"));
  let saved = read_value(&engine.paths.root.join("status/one.json")).expect("status");
  assert_eq!(saved["phase"], "ready");
}

#[test]
fn github_budget_idle_hour_keeps_rest_primary_flat_and_graphql_separate() {
  let fixture = Fixture::start().expect("fixture");
  rest_routes(&fixture);
  fixture
    .route(
      "/graphql",
      Reply::new(200, graphql_body(false)).header("X-RateLimit-Remaining", "4900"),
    )
    .expect("GraphQL route");
  let (_temp, mut engine, tick) = engine(&fixture);
  for slot in 0..=20 {
    engine.world.now_ms = slot * 180_000;
    engine.github_tick(&tick);
  }
  let requests = fixture.requests().expect("requests");
  let rest: Vec<_> = requests
    .iter()
    .filter(|request| request.path != "/graphql")
    .collect();
  let conditional = rest
    .iter()
    .filter(|request| request.headers.contains_key("if-none-match"))
    .count();
  assert_eq!(rest.len(), 126);
  assert!(conditional * 100 >= 95 * rest.len());
  assert_eq!(engine.world.rest_points, 6);
  assert_eq!(engine.world.graphql_points, 21);
  assert_scheduled_budget(&engine.world);
}

fn assert_scheduled_budget(world: &World) {
  let scheduled: Vec<_> = world
    .outbox
    .iter()
    .filter_map(|step| match step {
      Step::Journal(row) => Some(row),
      Step::Call { .. } | Step::Status { .. } | Step::Issue { .. } => None,
    })
    .filter(|row| row.name == "github-check" && row.note.contains("\"cause\":\"scheduled\""))
    .collect();
  assert_eq!(scheduled.len(), 20);
  let points: u64 = scheduled
    .iter()
    .map(|row| {
      serde_json::from_str::<Value>(&row.note).expect("note")["graphqlPoints"]
        .as_u64()
        .expect("points")
    })
    .sum();
  assert_eq!(points, 20);
  assert!(
    scheduled
      .iter()
      .all(|row| row.note.contains("\"restNotModified\":6"))
  );
}

#[test]
fn github_retry_after_suppresses_requests_until_the_next_fixed_slot() {
  let fixture = Fixture::start().expect("fixture");
  rest_routes(&fixture);
  fixture
    .sequence(
      "/repos/o/r/pulls/1",
      vec![
        Reply::new(429, r#"{"message":"secondary rate limit"}"#)
          .header("Retry-After", "60")
          .header("X-RateLimit-Remaining", "40"),
        Reply::new(200, pr_reply()).header("ETag", "\"pr\""),
        Reply::new(304, ""),
      ],
    )
    .expect("rate route");
  fixture
    .route("/graphql", Reply::new(200, graphql_body(false)))
    .expect("GraphQL route");
  let (_temp, mut engine, tick) = engine(&fixture);
  engine.github_tick(&tick);
  assert_eq!(engine.world.github_hold_until_ms, 60_000);
  let first = fixture.requests().expect("requests").len();
  for at in [1, 30_000, 59_999, 60_000, 179_999] {
    engine.world.now_ms = at;
    engine.github_tick(&tick);
  }
  assert_eq!(fixture.requests().expect("requests").len(), first);
  for at in [180_000, 360_000] {
    engine.world.now_ms = at;
    engine.github_tick(&tick);
  }
  assert_eq!(engine.world.watches["pr:o/r#1"].next_at_ms, 540_000);
  assert!(engine.world.outbox.iter().any(|step| matches!(step, Step::Journal(row) if row.name == "github-error" && row.note.contains("\"retryAfter\":60"))));
}

#[test]
fn github_retry_after_from_graphql_holds_other_pr_watches() {
  let fixture = Fixture::start().expect("fixture");
  rest_routes(&fixture);
  fixture
    .route(
      "/graphql",
      Reply::new(429, r#"{"message":"secondary rate limit"}"#)
        .header("Retry-After", "60")
        .header("X-RateLimit-Remaining", "40"),
    )
    .expect("GraphQL throttle");
  let (temp, mut engine, tick) = engine(&fixture);
  let mut other = Issue::blank("two", "epic", &temp.path().display().to_string(), 0);
  other.repo = "o/r".into();
  other.phase = "babysit".into();
  other.stage = "running".into();
  other.pr = Some(2);
  engine.world.issues.insert("two".into(), other);
  watch::sync(&mut engine.world);
  engine.github_tick(&tick);
  assert_eq!(engine.world.github_hold_until_ms, 60_000);
  assert_eq!(engine.world.graphql_remaining, Some(40));
  assert!(engine.world.watches["pr:o/r#2"].immediate);
  let count = fixture.requests().expect("requests").len();
  assert_eq!(count, 7);
  engine.world.now_ms = 59_999;
  engine.github_tick(&tick);
  assert_eq!(fixture.requests().expect("requests").len(), count);
  assert!(engine.world.outbox.iter().any(|step| matches!(step, Step::Journal(row) if row.name == "github-check" && row.note.contains("\"graphqlRetryAfter\":60") && row.note.contains("\"graphqlRemaining\":40"))));
}

#[test]
fn incomplete_rest_probe_retries_changed_pr_and_records_partial_cost() {
  let fixture = Fixture::start().expect("fixture");
  rest_routes(&fixture);
  let merged = r#"{"state":"closed","merged":true,"head":{"sha":"abc"},"base":{"ref":"main"}}"#;
  fixture
    .sequence(
      "/repos/o/r/pulls/1",
      vec![
        Reply::new(200, merged).header("ETag", "\"merged\""),
        Reply::new(200, merged).header("ETag", "\"merged\""),
      ],
    )
    .expect("PR route");
  fixture
    .sequence(
      "/repos/o/r/commits/abc/check-runs?per_page=100",
      vec![
        Reply::new(500, "{}"),
        Reply::new(200, r#"{"check_runs":[]}"#),
      ],
    )
    .expect("checks route");
  let (_temp, mut engine, tick) = engine(&fixture);
  engine.github_tick(&tick);
  assert_eq!(engine.world.issues["one"].stage, "running");
  assert_eq!(engine.world.rest_points, 1);
  assert!(engine.world.watches["pr:o/r#1"].etags.is_empty());
  assert!(engine.world.outbox.iter().any(|step| matches!(step, Step::Journal(row) if row.name == "github-error" && row.note.contains("\"restPoints\":1"))));
  engine.world.now_ms = 180_000;
  engine.github_tick(&tick);
  assert_eq!(engine.world.issues["one"].stage, "merged");
  let requests = fixture.requests().expect("requests");
  let pr_requests: Vec<_> = requests
    .iter()
    .filter(|request| request.path == "/repos/o/r/pulls/1")
    .collect();
  assert_eq!(pr_requests.len(), 2);
  assert!(!pr_requests[1].headers.contains_key("if-none-match"));
}

#[test]
fn partial_probe_rate_limit_keeps_the_latest_response_counters() {
  let fixture = Fixture::start().expect("fixture");
  rest_routes(&fixture);
  fixture
    .route(
      "/repos/o/r/pulls/1",
      Reply::new(200, pr_reply()).header("X-RateLimit-Remaining", "1200"),
    )
    .expect("PR route");
  fixture
    .route(
      "/repos/o/r/commits/abc/check-runs?per_page=100",
      Reply::new(429, r#"{"message":"secondary rate limit"}"#)
        .header("Retry-After", "60")
        .header("X-RateLimit-Remaining", "40"),
    )
    .expect("checks route");
  let (_temp, mut engine, tick) = engine(&fixture);
  engine.github_tick(&tick);
  assert_eq!(engine.world.rest_points, 1);
  assert_eq!(
    engine
      .world
      .rest_rate
      .as_ref()
      .and_then(|rate| rate.remaining),
    Some(40)
  );
  assert_eq!(engine.world.github_hold_until_ms, 60_000);
}

#[test]
fn a_rate_limit_rearms_unprocessed_watches_without_moving_their_deadlines() {
  let mut world = World::new(0);
  let key = "pr:Falconiere/toolu#501".to_owned();
  let mut watch = Watch::new(
    Kind::Pr {
      key: "toolu-447".to_owned(),
      repo: "Falconiere/toolu".to_owned(),
      number: 501,
    },
    0,
  );
  assert_eq!(watch.take_due(0), Some(Cause::Immediate));
  let deadline = watch.next_at_ms;
  world.watches.insert(key.clone(), watch);
  rearm(&mut world, &[(key.clone(), Cause::Scheduled)], 0);
  assert_eq!(world.watches.get(&key).expect("watch").next_at_ms, deadline);
  assert_eq!(
    world.watches.get_mut(&key).expect("watch").take_due(1),
    Some(Cause::Immediate)
  );
  assert_eq!(cause_name(Cause::Scheduled), "scheduled");
}
