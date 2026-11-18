use std::path::PathBuf;

use serde_json::{Value, json};

use super::evaluate;
use crate::Verdict;
use crate::options::Options;

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn outputs(docs: bool, rust: bool) -> Value {
  json!({
    "docs": if docs { "true" } else { "false" },
    "rust": if rust { "true" } else { "false" },
    "ports": "false",
    "changed": "true",
    "rust_release": "false"
  })
}

fn suite(changes_outputs: &Value, rust: &str, docs: &str) -> String {
  json!({
    "changes": { "result": "success", "outputs": changes_outputs },
    "docs": { "result": docs },
    "rust": { "result": rust },
    "rust-musl": { "result": "skipped" },
    "fuzz": { "result": "skipped" },
    "hook-bench": { "result": "skipped" }
  })
  .to_string()
}

#[test]
fn a_docs_only_run_passes() {
  let raw = suite(&outputs(true, false), "skipped", "success");
  let report = evaluate(&repo(), "tests.yml", Some(&raw)).unwrap();
  assert!(report.ok, "{:?}", report.lines);
  assert!(
    report
      .lines
      .iter()
      .any(|line| line == "docs (docs on): success")
  );
}

#[test]
fn a_failed_changes_job_exits_1() {
  let raw = json!({
    "changes": { "result": "failure" },
    "docs": { "result": "skipped" }
  })
  .to_string();
  let report = evaluate(&repo(), "tests.yml", Some(&raw)).unwrap();
  assert!(!report.ok);
  assert_eq!(
    report.lines,
    vec!["changes: failure; no group decision to trust".to_owned()]
  );
}

#[test]
fn the_rust_job_may_skip_off_the_release_branch() {
  let mut groups = outputs(false, true);
  groups["rust_release"] = json!("false");
  let raw = json!({
    "changes": { "result": "success", "outputs": groups },
    "docs": { "result": "skipped" },
    "rust": { "result": "skipped" },
    "rust-musl": { "result": "success" },
    "fuzz": { "result": "success" },
    "hook-bench": { "result": "skipped" }
  })
  .to_string();
  let report = evaluate(&repo(), "tests.yml", Some(&raw)).unwrap();
  assert!(report.ok, "{:?}", report.lines);
}

#[test]
fn the_rust_job_must_run_when_the_release_branch_is_on() {
  let mut groups = outputs(false, false);
  groups["rust_release"] = json!("true");
  let raw = suite(&groups, "skipped", "skipped");
  let report = evaluate(&repo(), "tests.yml", Some(&raw)).unwrap();
  assert_eq!(
    report.lines,
    vec!["rust (rust_release on): skipped, but its group is on".to_owned()]
  );
}

#[test]
fn a_job_skipped_while_its_group_is_on_fails() {
  let raw = suite(&outputs(true, false), "skipped", "skipped");
  let report = evaluate(&repo(), "tests.yml", Some(&raw)).unwrap();
  assert_eq!(
    report.lines,
    vec!["docs (docs on): skipped, but its group is on".to_owned()]
  );
}

#[test]
fn missing_or_unknown_needs_exits_1() {
  let missing = evaluate(&repo(), "tests.yml", None).unwrap();
  assert_eq!(
    missing.lines,
    vec!["ci-aggregate: NEEDS is not JSON".to_owned()]
  );
  let unknown = evaluate(&repo(), "nope.yml", Some("{}")).unwrap();
  assert_eq!(
    unknown.lines,
    vec!["ci-aggregate: nope.yml has no entry in the data file".to_owned()]
  );
  let verdict = super::run(&Options {
    root: repo(),
    ..Options::default()
  })
  .unwrap();
  assert_eq!(verdict, Verdict::Findings);
}
