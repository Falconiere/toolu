use std::os::unix::process::CommandExt as _;
use std::process::Command;

use nix::sys::signal::Signal;

use super::{alive, signal, terminate};

fn sleeper() -> std::process::Child {
  Command::new("sleep")
    .arg("30")
    .process_group(0)
    .spawn()
    .unwrap()
}

#[test]
fn a_running_group_is_alive_until_terminated() {
  let mut child = sleeper();
  let group = child.id();
  assert!(alive(group));
  terminate(group).unwrap();
  child.wait().unwrap();
  assert!(!alive(group));
}

#[test]
fn a_zombie_leader_does_not_count_as_alive() {
  let mut child = sleeper();
  let group = child.id();
  signal(group, Signal::SIGKILL).unwrap();
  std::thread::sleep(std::time::Duration::from_millis(100));
  assert!(!alive(group), "the unreaped leader is a zombie");
  child.wait().unwrap();
}

#[test]
fn signalling_an_exited_group_succeeds() {
  let mut child = sleeper();
  let group = child.id();
  terminate(group).unwrap();
  child.wait().unwrap();
  assert_eq!(signal(group, Signal::SIGTERM), Ok(()));
  assert_eq!(terminate(group), Ok(()));
}

#[test]
fn a_non_positive_or_huge_id_is_refused() {
  assert!(
    signal(0, Signal::SIGTERM)
      .unwrap_err()
      .contains("not a process-group id")
  );
  assert!(signal(u32::MAX, Signal::SIGTERM).is_err());
  assert!(!alive(0));
}
