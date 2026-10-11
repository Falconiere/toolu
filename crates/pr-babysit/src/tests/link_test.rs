use std::time::Duration;

use serde_json::json;
use tempfile::TempDir;
use toolu_engine::LinkError;
use toolu_engine::babysit::{TickDecision, TickRequest};
use toolu_http_test_support::Fixture;

use super::{decision, link_error, tick};
use crate::error::Failure;
use crate::recorded;
use crate::tick::Source;

#[test]
fn the_result_decision_maps_to_the_engine_decision() {
  assert_eq!(
    decision(&json!({"decision": "success"})),
    TickDecision::Success
  );
  assert_eq!(
    decision(&json!({"decision": "escalate"})),
    TickDecision::Escalate
  );
  assert_eq!(
    decision(&json!({"decision": "keep_going"})),
    TickDecision::KeepGoing
  );
  assert_eq!(decision(&json!({})), TickDecision::KeepGoing);
}

#[test]
fn a_rate_limit_keeps_its_wait_and_anything_else_fails_with_its_code() {
  let limited = Failure::new("api_error", "read 'pr' failed")
    .with("class", "rate_limited")
    .with("retryAfter", 60)
    .with("remaining", 0)
    .with("resetAt", 1_760_000_000);
  assert_eq!(
    link_error(&limited),
    LinkError::RateLimited {
      retry_after: Some(Duration::from_secs(60)),
      remaining: Some(0),
      reset_at: Some(1_760_000_000),
    }
  );
  let locked = Failure::new("locked", "the slot is locked");
  assert_eq!(
    link_error(&locked),
    LinkError::Failed("locked: the slot is locked".into())
  );
}

#[test]
fn a_scheduled_tick_reads_github_once_and_reports_decision_and_cost() {
  let dir = TempDir::new().unwrap();
  let fixture = Fixture::start().unwrap();
  let sha = recorded::snapshot()["head"]["sha"]
    .as_str()
    .unwrap()
    .to_owned();
  recorded::serve(
    &fixture,
    &[&sha, &sha],
    recorded::replies(&recorded::round()),
  );
  let request = TickRequest {
    repo: "Falconiere/toolu".into(),
    number: 115,
    state_file: dir.path().join("falconiere-toolu-115.json"),
    now: Some("2026-10-11T12:00:00Z".into()),
  };
  let source = Source::GitHub {
    config: recorded::config(&fixture),
    page_size: 2,
  };
  let report = tick(&request, source, &recorded::env(&fixture)).unwrap();
  assert_eq!(report.decision, TickDecision::Escalate);
  assert_eq!(report.result["decision"], "escalate");
  assert!(report.graphql.is_some_and(|usage| usage.points > 0));
}
