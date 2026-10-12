use serde_json::{Value, json};

use super::{
  RemovalIntent, finished, read_issue_record, removal_intent, terminal_cleanup_complete,
};
use crate::finish_rig::{layout, patch, read};
use crate::workload::OwnedWorkload;

#[test]
fn every_active_stage_is_a_valid_finish_record_and_bad_ones_are_named() {
  let l = layout();
  for stage in ["awaiting_merge", "replacing", "cleaning", "uncertain"] {
    patch(&l, &json!({"stage": stage}));
    assert_eq!(
      read_issue_record(&l.record, "k-1").expect(stage)["stage"],
      stage
    );
  }
  let path = l.record.display().to_string();
  for (fields, detail) in [
    (json!({"stage": "ready"}), "stage"),
    (json!({"stage": "running", "checkout": "relative"}), "paths"),
    (json!({"resource_root": "/r"}), "partial resource ownership"),
    (
      json!({"resource_root": "r", "lease_token": "t"}),
      "resource ownership",
    ),
    (
      json!({"resource_root": "/r", "lease_token": ""}),
      "resource ownership",
    ),
    (
      json!({"lease_token": "t", "removal_intent": {"version": 2}}),
      "removal intent",
    ),
  ] {
    let base = json!({"key": "k-1", "repo": "o/r", "branch": "b", "checkout": l.main, "worktree": l.wt, "stage": "running"});
    let mut body = base.as_object().cloned().expect("object");
    body.extend(fields.as_object().cloned().expect("fields"));
    if body.contains_key("lease_token") && !body.contains_key("resource_root") {
      body.insert("resource_root".into(), json!("/r"));
    }
    crate::finish_rig::write(&l.record, &Value::Object(body));
    assert_eq!(
      read_issue_record(&l.record, "k-1"),
      Err(format!("invalid issue record {path}: {detail}"))
    );
  }
  assert_eq!(
    read_issue_record(&l.record, "k-2"),
    Err(format!("invalid issue record {path}: key mismatch"))
  );
}

#[test]
fn a_removal_intent_round_trips_and_terminal_evidence_is_checked() {
  let l = layout();
  let intent = RemovalIntent {
    workspace_id: "w-1".into(),
    worktree: l.wt.display().to_string(),
    wip_ref: Some("refs/epic-wip/k-1".into()),
    leftover_files: vec![" M a.txt".into()],
    workload: vec![OwnedWorkload {
      pid: 7,
      group: 7,
      started: "linux:1".into(),
    }],
  };
  patch(
    &l,
    &json!({"removal_intent": intent.to_value("2026-10-11T12:00:00.000Z")}),
  );
  let record = read(&l.record).as_object().cloned().expect("object");
  assert_eq!(removal_intent(&record), Ok(Some(intent)));
  assert!(!terminal_cleanup_complete(&record));
  let mut done = record.clone();
  for (key, value) in [
    ("stage", json!("merged")),
    ("worktree_removed", json!(true)),
    ("branch_deleted", json!(true)),
    ("worktree", json!("/gone/wt")),
  ] {
    done.insert(key.into(), value);
  }
  assert!(terminal_cleanup_complete(&done));
  assert_eq!(finished(&done)["leftover_files"], json!([]));
  let mut bad = record;
  bad.insert("removal_intent".into(), json!({"version": 1, "workspace_id": "w", "worktree": "/w", "prepared_at": "never", "wip_ref": null, "leftover_files": [], "workload": []}));
  assert_eq!(
    removal_intent(&bad),
    Err("invalid removal intent".to_owned())
  );
}
