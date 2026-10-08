use serde_json::json;

use super::{DiffRange, resolve_range};

#[test]
fn a_push_without_a_parent_and_a_dispatch_run_everything() {
  let zero = "0".repeat(40);
  let head = "a".repeat(40);
  let event = json!({"before": zero, "after": head});
  match resolve_range("push", Some(&event)) {
    DiffRange::All(reason) => assert!(reason.contains("no previous commit"), "{reason}"),
    DiffRange::Range(spec) => panic!("range {spec}"),
  }
  match resolve_range("workflow_dispatch", None) {
    DiffRange::All(reason) => assert!(reason.contains("workflow_dispatch"), "{reason}"),
    DiffRange::Range(spec) => panic!("range {spec}"),
  }
}

#[test]
fn a_pull_request_without_shas_runs_everything() {
  match resolve_range("pull_request", Some(&json!({}))) {
    DiffRange::All(reason) => assert!(reason.contains("base/head"), "{reason}"),
    DiffRange::Range(spec) => panic!("range {spec}"),
  }
}
