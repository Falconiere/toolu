use serde_json::{Value, json};

use super::{Docs, actions_and_permissions, aggregates, review, rust_legs, triggers};
use crate::workflow_yaml::Findings;

fn found(run: impl FnOnce(&mut Findings)) -> Vec<String> {
  let mut found = Findings::default();
  run(&mut found);
  found.0
}

#[test]
fn a_path_filter_on_a_required_trigger_is_named() {
  let tests = json!({"on": {"push": {"branches": ["main"], "paths": ["x"]},
    "pull_request": {"branches": ["main"]}, "workflow_dispatch": null}});
  let docs = [("tests.yml".to_owned(), tests)];
  let lines = found(|f| triggers(&Docs(&docs), f));
  assert!(lines.contains(
    &"tests.yml must run on main push/PR and manual dispatch without path filters".to_owned()
  ));
}

#[test]
fn an_unpinned_action_and_an_extra_writer_are_named() {
  let doc = json!({"permissions": {"contents": "read"}, "jobs": {"build": {
    "permissions": {"contents": "write"},
    "steps": [{"uses": "actions/checkout@main"}, {"uses": "./local"}, {"uses": "a/b@v4"}]}}});
  assert_eq!(
    found(|f| actions_and_permissions("x.yml", &doc, f)),
    [
      "x.yml:build has unnecessary contents: write",
      "x.yml:build action actions/checkout@main is not version-pinned",
    ]
  );
  let open = json!({"jobs": {}});
  assert_eq!(
    found(|f| actions_and_permissions("y.yml", &open, f)),
    ["y.yml must declare least-privilege default contents permission"]
  );
}

#[test]
fn an_aggregate_missing_a_gated_job_is_named() {
  let job = |needs: Value| {
    json!({"name": "gate", "if": "always()", "needs": needs,
    "steps": [{"run": "cargo xtask ci-aggregate tests.yml"}]})
  };
  let all = json!(["changes", "docs", "fuzz", "hook-bench", "rust", "rust-musl"]);
  let mut tests = json!({"jobs": {"gate": job(all.clone()),
    "typescript": job(all), "changes": {"steps": [{"run": "cargo xtask ci-changes"}]}}});
  tests["jobs"]["typescript"]["name"] = json!("typescript");
  assert_eq!(found(|f| aggregates(&tests, f)), Vec::<String>::new());
  tests["jobs"]["gate"]["needs"] = json!(["changes"]);
  assert_eq!(
    found(|f| aggregates(&tests, f)),
    ["tests.yml:gate must aggregate every gated job with always()"]
  );
}

#[test]
fn rust_legs_and_review_name_what_is_missing() {
  let lines = found(|f| rust_legs(&json!({}), f));
  assert!(lines.contains(&"tests.yml:rust lacks rust-macos on macos-14".to_owned()));
  assert!(lines.contains(&"tests.yml:rust-musl lacks aarch64-unknown-linux-musl".to_owned()));
  let root = tempfile::tempdir().unwrap();
  let lines = found(|f| review(root.path(), &json!({}), f));
  assert!(lines.contains(&"toolu-review.yml Rust prompt file is missing".to_owned()));
  assert!(lines.contains(&"toolu-review.yml:review lacks EXCLUDE_GLOBS docs/cli/**".to_owned()));
}
