use serde_json::{Value, json};

use super::{Docs, actions_and_permissions, aggregates, review, rust_legs, triggers};
use crate::workflow_yaml::Findings;

fn found(run: impl FnOnce(&mut Findings)) -> Vec<String> {
  let mut found = Findings::default();
  run(&mut found);
  found.0
}

fn committed_tests_workflow() -> Value {
  let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
  let text = std::fs::read_to_string(root.join(".github/workflows/tests.yml")).unwrap();
  crate::ci_yaml::yaml_to_json(&text).unwrap()
}

#[test]
fn the_supported_macos_runner_executes_both_rust_gates() {
  let tests = committed_tests_workflow();
  assert_eq!(found(|f| rust_legs(&tests, f)), Vec::<String>::new());
}

#[test]
fn a_retired_macos_runner_is_rejected() {
  let mut tests = committed_tests_workflow();
  let mac = tests["jobs"]["rust"]["strategy"]["matrix"]["include"]
    .as_array_mut()
    .unwrap()
    .iter_mut()
    .find(|leg| leg["check"] == "rust-macos")
    .unwrap();
  mac["os"] = json!("macos-14");
  assert_eq!(
    found(|f| rust_legs(&tests, f)),
    ["tests.yml:rust lacks rust-macos on macos-15"]
  );
}

#[test]
fn macos_clippy_and_tests_cannot_be_skipped_by_a_stale_condition_or_missing_gate() {
  for (condition, run) in [
    (
      "matrix.os == 'macos-14'",
      "cargo xtask gate --only clippy --only tests",
    ),
    ("matrix.os == 'macos-15'", "cargo xtask gate --only clippy"),
    ("matrix.os == 'macos-15'", "cargo xtask gate --only tests"),
  ] {
    let mut tests = committed_tests_workflow();
    let mac = tests["jobs"]["rust"]["steps"]
      .as_array_mut()
      .unwrap()
      .iter_mut()
      .find(|step| step["name"] == "macOS clippy and Rust tests")
      .unwrap();
    mac["if"] = json!(condition);
    mac["run"] = json!(run);
    assert_eq!(
      found(|f| rust_legs(&tests, f)),
      ["tests.yml:rust macOS leg must run clippy and Rust tests"],
      "{condition}: {run}"
    );
  }
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
  let mut tests = json!({"jobs": {"gate": job(all),
    "changes": {"steps": [{"run": "cargo xtask ci-changes"}]}}});
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
  assert!(lines.contains(&"tests.yml:rust lacks rust-macos on macos-15".to_owned()));
  assert!(lines.contains(&"tests.yml:rust-musl lacks aarch64-unknown-linux-musl".to_owned()));
  let root = tempfile::tempdir().unwrap();
  let lines = found(|f| review(root.path(), &json!({}), f));
  assert!(lines.contains(&"toolu-review.yml Rust prompt file is missing".to_owned()));
  assert!(lines.contains(&"toolu-review.yml:review lacks EXCLUDE_GLOBS docs/cli/**".to_owned()));
}
