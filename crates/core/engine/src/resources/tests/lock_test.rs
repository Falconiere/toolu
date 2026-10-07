//! The `state.lock` protocol and atomic writes on real directories (`resources.test.ts`).

use std::os::unix::fs::PermissionsExt as _;
use std::time::Duration;

use toolu_runtime::json::ordered::Ordered;

use super::{
  acquire_lock, process_alive, read_json_file, whole, with_resource_lock, write_json_atomic,
};
use crate::ledger::jq::parse_json;

fn owner(pid: &str, token: &str) -> Ordered {
  parse_json(&format!(r#"{{"pid":{pid},"token":"{token}"}}"#)).unwrap()
}

#[test]
fn a_lock_is_exclusive_token_safe_and_recovers_a_dead_owner() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("resource.lock");
  let first = acquire_lock(&path, Duration::ZERO).unwrap().unwrap();
  assert_eq!(
    acquire_lock(&path, Duration::ZERO)
      .unwrap()
      .map(|l| l.token().to_owned()),
    None
  );

  let me = std::process::id().to_string();
  write_json_atomic(&path.join("owner.json"), &owner(&me, "replacement")).unwrap();
  first.release();
  assert!(path.exists(), "a replaced owner keeps the lock");
  std::fs::remove_dir_all(&path).unwrap();

  std::fs::create_dir(&path).unwrap();
  write_json_atomic(&path.join("owner.json"), &owner("2147483647", "dead")).unwrap();
  let recovered = acquire_lock(&path, Duration::ZERO).unwrap().unwrap();
  recovered.release();
  assert!(!path.exists());
}

#[test]
fn a_crashed_reclaimer_fails_closed_and_corrupt_ownership_is_kept() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("state.lock");
  std::fs::create_dir(dir.path().join("state.lock.reap")).unwrap();
  assert!(
    acquire_lock(&path, Duration::from_millis(100))
      .unwrap()
      .is_none()
  );
  std::fs::remove_dir(dir.path().join("state.lock.reap")).unwrap();

  std::fs::create_dir(&path).unwrap();
  std::fs::write(path.join("owner.json"), "{\"pid\":\"x\"}").unwrap();
  assert!(
    acquire_lock(&path, Duration::from_millis(60))
      .unwrap()
      .is_none()
  );
  std::fs::write(path.join("owner.json"), "{not json").unwrap();
  assert!(acquire_lock(&path, Duration::ZERO).is_err());
}

#[test]
fn an_update_waits_for_the_lock_then_reports_busy() {
  let dir = tempfile::tempdir().unwrap();
  let held = acquire_lock(&dir.path().join("state.lock"), Duration::ZERO)
    .unwrap()
    .unwrap();
  let busy = with_resource_lock(dir.path(), || Ok(()));
  assert_eq!(busy, Err("resource state busy; retry later".to_owned()));
  held.release();
  assert_eq!(with_resource_lock(dir.path(), || Ok(7)), Ok(7));
  assert_eq!(
    with_resource_lock(dir.path(), || Err::<(), _>("inner".to_owned())),
    Err("inner".to_owned())
  );
  assert!(!dir.path().join("state.lock").exists());
}

#[test]
fn json_is_written_pretty_0600_and_read_back_with_a_fallback() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("nested/state.json");
  let value = parse_json(r#"{"version":1,"leases":[],"n":1.5}"#).unwrap();
  write_json_atomic(&file, &value).unwrap();
  assert_eq!(
    std::fs::read_to_string(&file).unwrap(),
    "{\n  \"version\": 1,\n  \"leases\": [],\n  \"n\": 1.5\n}\n"
  );
  assert_eq!(
    std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
    0o600
  );
  assert_eq!(
    std::fs::read_dir(file.parent().unwrap()).unwrap().count(),
    1
  );
  assert_eq!(read_json_file(&file, Ordered::Null), Ok(value.clone()));
  assert_eq!(
    read_json_file(&dir.path().join("absent"), Ordered::Bool(true)),
    Ok(Ordered::Bool(true))
  );
  std::fs::write(dir.path().join("bad.json"), "{").unwrap();
  assert!(read_json_file(&dir.path().join("bad.json"), Ordered::Null).is_err());
  assert!(write_json_atomic(&dir.path().join("bad.json/x"), &value).is_err());
}

#[test]
fn process_probes_need_a_whole_positive_pid() {
  assert_eq!(process_alive(f64::from(std::process::id())), Ok(true));
  assert_eq!(process_alive(2_147_483_647.0), Ok(false));
  assert_eq!(process_alive(1.5), Ok(false));
  assert_eq!(process_alive(-4.0), Ok(false));
  assert_eq!(whole(42.0), Some(42));
  assert_eq!(whole(0.5), None);
  assert_eq!(whole(1e30), None);
}
