use std::collections::BTreeSet;

use serde_json::json;
use toolu_http_test_support::{Fixture, Reply};

use super::{HOLD_AT, check, heuristic, migrations, path_tokens};
use crate::graph::GraphIssue;
use crate::support::{graph_248, jev};

fn issue(key: &str, title: &str, excerpt: &str) -> GraphIssue {
  let mut issue = graph_248().issues[0].clone();
  issue.key = key.into();
  issue.title = title.into();
  issue.excerpt = excerpt.into();
  issue.labels = Vec::new();
  issue
}

fn set(items: &[&str]) -> BTreeSet<String> {
  items.iter().map(|item| (*item).to_owned()).collect()
}

fn jev_reply(fixture: &Fixture, answers: &serde_json::Value) {
  let body =
    json!({"model": "m", "usage": {"input_tokens": 1, "output_tokens": 1}, "answers": answers});
  fixture
    .route("/v1/systemone", Reply::new(200, body.to_string()))
    .expect("route");
}

#[test]
fn file_paths_match_but_slashed_words_and_urls_do_not() {
  let text = "Edit src/db/users.rs and crates/core/http, lib.rs/x.ts. Use and/or CI/CD read/write.";
  assert_eq!(
    path_tokens(text),
    set(&["crates/core/http", "lib.rs/x.ts", "src/db/users.rs"])
  );
  assert_eq!(
    path_tokens("See https://github.com/acme/api/blob/main/src/a.rs for docs"),
    BTreeSet::new()
  );
  assert_eq!(
    path_tokens("Ends at docs/guide.md."),
    set(&["docs/guide.md"])
  );
}

#[test]
fn migration_identifiers_match_both_shapes() {
  assert_eq!(
    migrations("Add 0042_add_users and V3__init_schema, not 42_x or v3__y"),
    set(&["0042_add_users", "V3__init_schema"])
  );
}

#[test]
fn two_issues_naming_the_same_migration_overlap_without_jev() {
  let a = issue("a-1", "Add users table", "Write migration 0042_add_users.");
  let b = issue(
    "b-1",
    "Backfill users",
    "Depends on 0042_add_users being applied.",
  );
  assert_eq!(
    heuristic(&a, &b).as_deref(),
    Some("shared migration 0042_add_users")
  );
  let verdict = check(&b, &[&a], None);
  assert_eq!(
    verdict.hold.map(|hold| (hold.with, hold.source)),
    Some(("a-1".to_owned(), "heuristic"))
  );
}

#[test]
fn unrelated_issues_and_slashed_words_or_links_do_not_overlap() {
  let a = issue(
    "a-1",
    "Improve CI/CD and/or docs",
    "See https://github.com/acme/api/blob/main/src/a.rs",
  );
  let b = issue(
    "b-1",
    "Tune read/write and/or caching",
    "See https://github.com/acme/api/blob/main/src/a.rs",
  );
  assert_eq!(heuristic(&a, &b), None);
  assert_eq!(check(&b, &[&a], None).hold, None);
}

#[test]
fn shared_paths_and_scoped_labels_overlap() {
  let a = issue("a-1", "Refactor src/http/client.rs", "");
  let b = issue("b-1", "Retry in src/http/client.rs", "");
  assert_eq!(
    heuristic(&a, &b).as_deref(),
    Some("shared path src/http/client.rs")
  );
  let (mut c, mut d) = (issue("c-1", "One", ""), issue("d-1", "Two", ""));
  c.labels = vec!["module:billing".into(), "bug".into()];
  d.labels = vec!["module:billing".into()];
  assert_eq!(
    heuristic(&c, &d).as_deref(),
    Some("shared label module:billing")
  );
  d.labels = vec!["bug".into()];
  assert_eq!(heuristic(&c, &d), None);
}

#[test]
fn a_lone_candidate_has_nothing_to_compare() {
  let a = issue("a-1", "Add users", "0042_add_users");
  assert_eq!(check(&a, &[], None), super::Verdict::default());
}

#[test]
fn jev_holds_a_pair_at_or_above_the_threshold() {
  let fixture = Fixture::start().expect("fixture");
  jev_reply(&fixture, &json!({"a-1": {"type": "noul", "noul": 0.93}}));
  let (a, b) = (
    issue("a-1", "Users table", "schema"),
    issue("b-1", "Users API", "schema"),
  );
  let client = jev(&fixture);
  let verdict = check(&b, &[&a], Some(Ok(&client)));
  let hold = verdict.hold.expect("hold");
  assert_eq!(
    (hold.with.as_str(), hold.score, hold.source),
    ("a-1", 0.93, "jev")
  );
  assert!(hold.score >= HOLD_AT && verdict.fallback.is_none());
  let sent: serde_json::Value =
    serde_json::from_slice(&fixture.requests().expect("requests")[0].body).expect("json");
  assert!(
    sent
      .to_string()
      .contains("change the same module, schema or migration")
  );
}

#[test]
fn jev_below_the_threshold_does_not_hold_even_when_the_heuristic_would() {
  let fixture = Fixture::start().expect("fixture");
  jev_reply(&fixture, &json!({"a-1": {"type": "noul", "noul": 0.4}}));
  let a = issue("a-1", "Add users", "0042_add_users");
  let b = issue("b-1", "Backfill users", "0042_add_users");
  let client = jev(&fixture);
  assert_eq!(
    check(&b, &[&a], Some(Ok(&client))),
    super::Verdict::default()
  );
}

#[test]
fn a_jev_failure_falls_back_to_the_heuristic_with_its_reason() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/v1/systemone", Reply::new(500, "{}"))
    .expect("route");
  let a = issue("a-1", "Add users", "0042_add_users");
  let b = issue("b-1", "Backfill users", "0042_add_users");
  let client = jev(&fixture);
  let verdict = check(&b, &[&a], Some(Ok(&client)));
  assert_eq!(verdict.hold.map(|hold| hold.source), Some("heuristic"));
  assert!(
    verdict
      .fallback
      .is_some_and(|reason| reason.starts_with("jev failed"))
  );
  let unavailable = check(&b, &[&a], Some(Err("TYPESAFE_API_KEY not set".into())));
  assert_eq!(
    unavailable.fallback.as_deref(),
    Some("TYPESAFE_API_KEY not set")
  );
}

#[test]
fn the_first_overlapping_member_in_batch_order_wins() {
  let a = issue("a-1", "Docs", "");
  let b = issue("b-1", "Edit src/x.rs", "");
  let c = issue("c-1", "Also src/x.rs", "");
  let d = issue("d-1", "Again src/x.rs", "");
  assert_eq!(
    check(&d, &[&a, &b, &c], None).hold.map(|hold| hold.with),
    Some("b-1".to_owned())
  );
}
