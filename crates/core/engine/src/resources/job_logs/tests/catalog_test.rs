use std::fs::{self, OpenOptions};
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::list;
use crate::resources::job_logs::start;
use crate::resources::leases::{LeaseKind, LeaseRequest, acquire_lease, release_lease};

fn modified_at(path: &Path, time: SystemTime) {
  OpenOptions::new()
    .write(true)
    .open(path)
    .unwrap()
    .set_modified(time)
    .unwrap();
}

#[test]
fn retention_removes_old_and_excess_done_logs() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  for index in 0..21 {
    start(
      root,
      &format!("done-{index:02}"),
      "worker",
      &["true".to_owned()],
    )
    .unwrap();
  }
  assert_eq!(list(root, "worker").unwrap().len(), 20);
  let old = start(root, "old", "other", &["true".to_owned()]).unwrap();
  let stale = SystemTime::now() - Duration::from_hours(25);
  modified_at(&old.path, stale);
  assert!(list(root, "other").unwrap().is_empty());
  assert!(!root.join("jobs/logs/old.lock").exists());
}

#[test]
fn retention_exempts_a_live_lease() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let stale = SystemTime::now() - Duration::from_hours(25);
  let req = LeaseRequest {
    kind: LeaseKind::Job,
    key: "worker:live".to_owned(),
    state_dir: "epic".to_owned(),
    host: None,
    host_cap: None,
    epic_cap: None,
    worktree: None,
  };
  let lease = acquire_lease(root, &req, SystemTime::now()).unwrap();
  let token = match lease.get("token") {
    Some(toolu_runtime::json::ordered::Ordered::String(value)) => value.clone(),
    _ => panic!("lease token missing"),
  };
  let live = start(root, &token, "worker", &["sleep".to_owned()]).unwrap();
  for index in 0..20 {
    start(
      root,
      &format!("done-{index:02}"),
      "worker",
      &["true".to_owned()],
    )
    .unwrap();
  }
  let mut meta: serde_json::Value = serde_json::from_slice(&fs::read(&live.meta).unwrap()).unwrap();
  meta.as_object_mut().unwrap().insert(
    "started_at".to_owned(),
    serde_json::Value::String("9999-01-01T00:00:00Z".to_owned()),
  );
  fs::write(&live.meta, meta.to_string()).unwrap();
  modified_at(&live.path, stale);
  assert!(
    list(root, "worker")
      .unwrap()
      .iter()
      .any(|log| log.id == format!("job-{token}") && log.state == "running")
  );
  assert_eq!(list(root, "worker").unwrap().len(), 21);
  release_lease(root, &token).unwrap();
  assert!(
    !list(root, "worker")
      .unwrap()
      .iter()
      .any(|log| log.id == format!("job-{token}"))
  );
}
