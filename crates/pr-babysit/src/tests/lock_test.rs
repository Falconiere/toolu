use std::path::Path;

use serde_json::Value;
use toolu_runtime::env::Env;

use super::SlotLock;

fn lock_dir(state: &Path) -> std::path::PathBuf {
  state.with_file_name("s.json.lock")
}

#[test]
fn a_held_slot_is_locked_with_its_holder_and_released_on_drop() {
  let dir = tempfile::tempdir().unwrap();
  let state = dir.path().join("nested/s.json");
  let env = Env::default();
  let held = SlotLock::acquire(&state, &env).unwrap();
  let pid = std::fs::read_to_string(lock_dir(&state).join("pid")).unwrap();
  assert_eq!(pid.trim(), std::process::id().to_string());
  let err = SlotLock::acquire(&state, &env).unwrap_err();
  assert_eq!(err.code, "locked");
  assert_eq!(err.extra["pid"], Value::from(std::process::id()));
  assert!(err.extra["since"].is_u64());
  drop(held);
  assert!(!lock_dir(&state).exists());
  let again = SlotLock::acquire(&state, &env).unwrap();
  drop(again);
}

#[test]
fn a_dead_holder_or_an_old_lock_is_reclaimed() {
  let dir = tempfile::tempdir().unwrap();
  let state = dir.path().join("s.json");
  let lock = lock_dir(&state);
  std::fs::create_dir(&lock).unwrap();
  std::fs::write(lock.join("pid"), "999999\n").unwrap();
  std::fs::write(lock.join("since"), "1\n").unwrap();
  let reclaimed = SlotLock::acquire(&state, &Env::default()).unwrap();
  drop(reclaimed);
  std::fs::create_dir(&lock).unwrap();
  std::fs::write(lock.join("pid"), format!("{}\n", std::process::id())).unwrap();
  std::fs::write(lock.join("since"), "1\n").unwrap();
  let fresh_limit = Env::from_pairs([("PB_LOCK_STALE_SECONDS", "600")]);
  let old = SlotLock::acquire(&state, &fresh_limit).unwrap();
  drop(old);
}

#[test]
fn a_young_incomplete_lock_is_contention_and_a_replacement_survives_release() {
  let dir = tempfile::tempdir().unwrap();
  let state = dir.path().join("s.json");
  let lock = lock_dir(&state);
  std::fs::create_dir(&lock).unwrap();
  let err = SlotLock::acquire(&state, &Env::default()).unwrap_err();
  assert_eq!(err.code, "locked");
  assert_eq!(err.extra["pid"], Value::Null);
  std::fs::remove_dir(&lock).unwrap();
  let held = SlotLock::acquire(&state, &Env::default()).unwrap();
  std::fs::remove_dir_all(&lock).unwrap();
  std::fs::create_dir(&lock).unwrap();
  drop(held);
  assert!(
    lock.exists(),
    "release never deletes another holder's directory"
  );
}

#[test]
fn a_live_holder_within_the_limit_is_not_stale() {
  let dir = tempfile::tempdir().unwrap();
  let state = dir.path().join("s.json");
  let lock = lock_dir(&state);
  std::fs::create_dir(&lock).unwrap();
  std::fs::write(lock.join("pid"), format!("{}\n", std::process::id())).unwrap();
  let since = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_secs();
  std::fs::write(lock.join("since"), format!("{since}\n")).unwrap();
  let err =
    SlotLock::acquire(&state, &Env::from_pairs([("PB_LOCK_STALE_SECONDS", "x")])).unwrap_err();
  assert_eq!(err.code, "locked");
  assert_eq!(err.extra["since"], Value::from(since));
}
