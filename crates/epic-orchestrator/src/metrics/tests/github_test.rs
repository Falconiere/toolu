use serde_json::json;

use super::GitHub;
use crate::journal::Record;
use crate::metrics::scope::Scope;

fn check(watch: &str, note: &serde_json::Value) -> Record {
  Record::new("transition", "github-check", watch, "", &note.to_string())
}

fn scheduled(next: u64) -> serde_json::Value {
  json!({"cause": "scheduled", "restFresh": 1, "restNotModified": 3, "restPoints": 1,
    "graphqlPoints": 2, "nextAt": next})
}

/// Four scheduled attempts, a held error, an engine restart and an extra check.
fn timeline() -> serde_json::Value {
  let scope = Scope::new(None);
  let mut github = GitHub::default();
  github.push(&scope, "i", &check("pr:o/r#1", &scheduled(180_000)), 0);
  github.push(
    &scope,
    "i",
    &check("pr:o/r#1", &scheduled(360_000)),
    180_010,
  );
  let held = json!({"cause": "scheduled", "error": "x", "retryAfter": 60, "nextAt": 540_000});
  let error = Record::new(
    "transition",
    "github-error",
    "pr:o/r#1",
    "",
    &held.to_string(),
  );
  github.push(&scope, "i", &error, 360_000);
  github.push(
    &scope,
    "i",
    &check("pr:o/r#1", &scheduled(900_000)),
    720_000,
  );
  let start = Record::new("action", "engine-usage", "", "", r#"{"event":"start"}"#);
  github.push(&scope, "", &start, 730_000);
  let plain = json!({"cause": "scheduled"});
  github.push(&scope, "i", &check("pr:o/r#1", &plain), 800_000);
  github.push(
    &scope,
    "i",
    &check("pr:o/r#1", &json!({"cause": "immediate"})),
    800_001,
  );
  github.finish()
}

#[test]
fn counts_requests_points_and_the_share_answered_not_modified() {
  let doc = timeline();
  assert_eq!(doc["checks"], 5);
  assert_eq!(doc["errors"], 1);
  assert_eq!(doc["rest_not_modified"], 9);
  assert_eq!(doc["not_modified_share"], json!(0.75));
  assert_eq!(doc["graphql_points"], 6);
}

#[test]
fn gaps_pair_scheduled_attempts_of_one_lifetime_without_a_hold() {
  let doc = timeline();
  assert_eq!(doc["next_gap_ms"]["max"], 180_000);
  assert_eq!(
    doc["check_gap_ms"],
    json!({"count": 2, "p50": 180_000, "p90": 180_008, "max": 180_010})
  );
}

#[test]
fn a_reaction_is_late_only_when_it_waited_for_a_check_with_a_free_queue() {
  let mut scope = Scope::new(None);
  for (watch, issue) in [("pr:o/r#1", "a"), ("pr:o/r#2", "b"), ("pr:o/s#3", "c")] {
    scope.learn(&check(watch, &json!({"issue": issue})));
  }
  let mut github = GitHub::default();
  let ready = |issue: &str| Record::new("transition", "github-babysit-ready", issue, "", "");
  let intent = |issue: &str| Record::new("action", "merge-intent", issue, "", "");
  github.push(&scope, "a", &ready("a"), 0);
  github.push(&scope, "a", &intent("a"), 5);
  github.push(&scope, "b", &ready("b"), 10);
  github.push(&scope, "c", &ready("c"), 10);
  github.push(&scope, "b", &check("pr:o/r#2", &json!({})), 20);
  github.push(&scope, "c", &check("pr:o/s#3", &json!({})), 20);
  github.push(&scope, "c", &intent("c"), 30);
  github.push(&scope, "a", &ready("a"), 40);
  github.push(&scope, "a", &check("pr:o/r#1", &json!({})), 50);
  github.push(&scope, "b", &intent("b"), 60);
  let doc = github.finish();
  assert_eq!(doc["reaction_ms"]["count"], 3);
  assert_eq!(
    doc["late_reactions"], 2,
    "c waited a check; b waited with a's repo free"
  );
}
