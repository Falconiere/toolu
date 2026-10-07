use std::os::unix::process::CommandExt as _;
use std::process::Command;

use nix::sys::signal::Signal;

use super::{alive, signal, signal_probe, terminate, terminate_reaping};

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

#[test]
fn a_reaped_leader_leaves_a_group_the_signal_probe_alone_calls_dead() {
  let mut child = sleeper();
  let group = child.id();
  assert!(signal_probe(group));
  signal(group, Signal::SIGKILL).unwrap();
  child.wait().unwrap();
  assert!(!signal_probe(group));
  assert!(!signal_probe(0));
}

#[test]
fn terminate_reaping_calls_the_reaper_and_ends_the_leader() {
  use std::os::unix::process::ExitStatusExt as _;
  let mut child = sleeper();
  let group = child.id();
  let mut calls = 0;
  terminate_reaping(group, &mut || calls += 1).unwrap();
  assert!(calls >= 1);
  assert_eq!(child.wait().unwrap().signal(), Some(15));
}

#[test]
fn pid_alive_probes_a_process_and_rejects_impossible_ids() {
  use super::pid_alive;
  let mut child = sleeper();
  let pid = i64::from(child.id());
  assert_eq!(pid_alive(pid), Ok(true));
  terminate(child.id()).unwrap();
  child.wait().unwrap();
  assert_eq!(pid_alive(pid), Ok(false));
  assert_eq!(pid_alive(0), Ok(false));
  assert_eq!(pid_alive(-3), Ok(false));
  assert_eq!(pid_alive(i64::from(i32::MAX) + 1), Ok(false));
  assert_eq!(pid_alive(i64::from(std::process::id())), Ok(true));
}
