use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::process::Command;
use std::time::Duration;

use super::GroupGuard;
use crate::process::group;

fn sleeper() -> std::process::Child {
  Command::new("sleep")
    .arg("30")
    .process_group(0)
    .spawn()
    .unwrap()
}

// One test: the armed slot is process-wide, so parallel tests would race on it.
#[test]
fn a_guard_disarms_when_dropped_and_kills_its_group_only_while_unwinding() {
  let mut kept = sleeper();
  let guard = GroupGuard::install().unwrap();
  assert_eq!(guard.armed(), 0);
  guard.arm(kept.id());
  assert_eq!(guard.armed(), kept.id());
  drop(guard);
  assert_eq!(GroupGuard::install().unwrap().armed(), 0);
  assert!(
    group::alive(kept.id()),
    "a normal drop leaves the group running"
  );

  let mut killed = sleeper();
  let id = killed.id();
  let unwound = std::panic::catch_unwind(move || {
    let guard = GroupGuard::install().unwrap();
    guard.arm(id);
    panic!("the runner fails mid-check");
  });
  assert!(unwound.is_err());
  std::thread::sleep(Duration::from_millis(100));
  assert_eq!(killed.wait().unwrap().signal(), Some(9));
  assert_eq!(GroupGuard::install().unwrap().armed(), 0);

  group::terminate(kept.id()).unwrap();
  kept.wait().unwrap();
}
