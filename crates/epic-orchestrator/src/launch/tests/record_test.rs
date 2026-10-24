use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{load_launch_record, write_record};

fn put(dir: &Path, name: &str, body: &str) -> PathBuf {
  let path = dir.join(name);
  std::fs::write(&path, body).expect("record");
  path
}

fn refused(dir: &Path, body: &Value) -> String {
  load_launch_record(&put(dir, "r.json", &body.to_string())).expect_err("refused")
}

#[test]
fn a_missing_record_is_a_new_attempt_and_fields_are_kept() {
  let tmp = tempfile::tempdir().expect("temp");
  assert!(
    load_launch_record(&tmp.path().join("missing.json"))
      .expect("missing")
      .is_empty()
  );
  let body = json!({
    "stage": "running", "launches": 2, "lease_token": "lease-1", "resource_root": "/r",
    "pane_id": "p", "worktree": "/w", "session_id": "s", "session_started_after": 1_797_863_400_000_u64,
    "prompt_state": "acknowledged", "extension": {"durable": true},
  });
  let record = load_launch_record(&put(tmp.path(), "ok.json", &body.to_string())).expect("valid");
  assert_eq!(Value::Object(record.clone()), body);
  write_record(&tmp.path().join("deep/out.json"), &record).expect("write");
  assert_eq!(
    load_launch_record(&tmp.path().join("deep/out.json")).map(Value::Object),
    Ok(body)
  );
}

#[test]
fn malformed_and_non_object_records_fail_closed() {
  let tmp = tempfile::tempdir().expect("temp");
  for (source, detail) in [
    ("{\"stage\":", "malformed JSON"),
    ("[]", "expected a JSON object"),
    ("null", "expected a JSON object"),
    ("\"running\"", "expected a JSON object"),
    ("2", "expected a JSON object"),
  ] {
    let path = put(tmp.path(), "bad.json", source);
    assert_eq!(
      load_launch_record(&path),
      Err(format!(
        "invalid launch record {}: {detail}",
        path.display()
      ))
    );
  }
}

#[test]
fn only_known_stages_are_accepted() {
  let tmp = tempfile::tempdir().expect("temp");
  for stage in [
    "uncertain",
    "replacing",
    "starting",
    "cleaning",
    "cleanup-incomplete",
    "running",
    "awaiting_merge",
    "merged",
    "abandoned",
  ] {
    let path = put(tmp.path(), "s.json", &json!({"stage": stage}).to_string());
    assert_eq!(load_launch_record(&path).expect("stage")["stage"], stage);
  }
  for stage in [json!("ready"), json!(""), Value::Null, json!(3)] {
    assert!(refused(tmp.path(), &json!({"stage": stage})).contains("unknown stage"));
  }
}

#[test]
fn launches_and_the_session_epoch_must_be_well_formed() {
  let tmp = tempfile::tempdir().expect("temp");
  for launches in [0, 1, 42] {
    let path = put(
      tmp.path(),
      "l.json",
      &json!({"launches": launches}).to_string(),
    );
    assert_eq!(
      load_launch_record(&path).expect("launches")["launches"],
      launches
    );
  }
  for launches in [
    json!(-1),
    json!(0.5),
    json!("1"),
    Value::Null,
    json!(9_007_199_254_740_992_u64),
  ] {
    assert!(
      refused(tmp.path(), &json!({"launches": launches}))
        .ends_with("launches must be a nonnegative integer")
    );
  }
  for since in [json!(-1), json!("0"), Value::Null] {
    assert!(
      refused(tmp.path(), &json!({"session_started_after": since}))
        .ends_with("session_started_after must be a finite epoch")
    );
  }
  // JSON has no infinity: a number past f64 is malformed before it is an epoch.
  let infinite = put(tmp.path(), "i.json", r#"{"session_started_after":1e400}"#);
  assert!(
    load_launch_record(&infinite)
      .expect_err("infinite")
      .ends_with("malformed JSON")
  );
}

#[test]
fn identity_fields_must_be_non_empty_strings() {
  let tmp = tempfile::tempdir().expect("temp");
  for field in [
    "lease_token",
    "resource_root",
    "pane_id",
    "worktree",
    "session_id",
  ] {
    for value in [
      json!(3),
      json!(false),
      json!({}),
      json!([]),
      json!(""),
      json!("  "),
    ] {
      let err = refused(tmp.path(), &json!({field: value}));
      assert!(
        err.ends_with(&format!("{field} must be a non-empty string")),
        "{err}"
      );
    }
  }
}

#[test]
fn a_pending_session_may_be_null_but_an_acknowledged_one_may_not() {
  let tmp = tempfile::tempdir().expect("temp");
  let pending = put(
    tmp.path(),
    "p.json",
    &json!({"stage": "starting", "prompt_state": "submitting", "session_id": null}).to_string(),
  );
  assert_eq!(
    load_launch_record(&pending).expect("pending")["session_id"],
    Value::Null
  );
  let acknowledged =
    json!({"stage": "running", "prompt_state": "acknowledged", "session_id": null});
  assert!(
    refused(tmp.path(), &acknowledged)
      .ends_with("session_id may be null only while awaiting capture")
  );
  let managed =
    json!({"stage": "running", "prompt_state": "acknowledged", "lease_token": "lease-1"});
  assert!(refused(tmp.path(), &managed).ends_with("a managed running launch requires session_id"));
  let legacy = put(
    tmp.path(),
    "legacy.json",
    &json!({"stage": "running", "prompt_state": "acknowledged"}).to_string(),
  );
  assert!(
    load_launch_record(&legacy)
      .expect("legacy")
      .get("session_id")
      .is_none()
  );
  for stage in ["cleaning", "cleanup-incomplete", "merged", "abandoned"] {
    let body = json!({"stage": stage, "lease_token": "lease-1", "prompt_state": "acknowledged", "session_id": null, "cleanup_error": "owned workload remains", "leftover_files": [" M src/a.ts"]});
    let record =
      load_launch_record(&put(tmp.path(), "t.json", &body.to_string())).expect("terminal");
    assert_eq!(record["cleanup_error"], "owned workload remains");
  }
}

/// The record #625 left: the engine adopted a reporting worker as running
/// before its launch confirmed the prompt or captured the session.
#[test]
fn an_adopted_running_record_may_lack_its_session_until_acknowledged() {
  let tmp = tempfile::tempdir().expect("temp");
  let adopted = json!({
    "stage": "running", "prompt_state": "submitting", "lifecycle_outcome": "uncertain",
    "session_id": null, "lease_token": "lease-1", "resource_root": "/r", "pane_id": "p",
    "worktree": "/w",
  });
  let record = load_launch_record(&put(tmp.path(), "a.json", &adopted.to_string()));
  assert_eq!(record.map(Value::Object), Ok(adopted));
  for body in [
    json!({"stage": "running", "prompt_state": "acknowledged", "session_id": null}),
    json!({"stage": "running", "session_id": null}),
  ] {
    let err = refused(tmp.path(), &body);
    assert!(
      err.ends_with("session_id may be null only while awaiting capture"),
      "{body}: {err}"
    );
  }
  let managed =
    json!({"stage": "running", "prompt_state": "acknowledged", "lease_token": "lease-1"});
  assert!(refused(tmp.path(), &managed).ends_with("a managed running launch requires session_id"));
}
