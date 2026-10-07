use std::fs;

use super::Held;

#[test]
fn a_live_lock_is_busy_and_a_dead_pid_is_reclaimed() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("engine.lock");
  let held = Held::acquire(&path, "engine-busy").unwrap();
  assert_eq!(
    Held::acquire(&path, "engine-busy").unwrap_err(),
    "engine-busy"
  );
  drop(held);
  assert!(!path.exists());
  let mut child = std::process::Command::new("true").spawn().unwrap();
  let pid = child.id();
  child.wait().unwrap();
  fs::write(&path, format!("{pid}\n")).unwrap();
  let held = Held::acquire(&path, "engine-busy").unwrap();
  assert!(path.exists());
  drop(held);
}
