use std::os::unix::process::CommandExt as _;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use super::{kill, kill_stragglers, retry_busy, spawn_retrying, wait};

fn leader(script: &str) -> Child {
  Command::new("/bin/sh")
    .args(["-c", script])
    .process_group(0)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .spawn()
    .unwrap()
}

#[test]
fn a_child_that_exits_is_reported_with_its_status_and_ticks() {
  let mut child = leader("exit 3");
  let mut ticks = 0;
  let status = wait(&mut child, Duration::from_secs(10), &mut || ticks += 1).unwrap();
  assert_eq!(status.and_then(|status| status.code()), Some(3));
  assert!(ticks >= 2, "ticks {ticks}");
}

#[test]
fn a_child_past_its_deadline_is_killed_with_its_whole_group() {
  let mut child = leader("sleep 30 & sleep 30");
  let started = Instant::now();
  let status = wait(&mut child, Duration::from_millis(100), &mut || {}).unwrap();
  assert!(status.is_none());
  assert!(started.elapsed() < Duration::from_secs(10));
  // The group is gone: killing it again is still success.
  kill(child.id()).unwrap();
}

#[test]
fn a_group_that_is_gone_is_success_and_a_bad_id_is_an_error() {
  let mut child = leader("exit 0");
  let id = child.id();
  child.wait().unwrap();
  kill(id).unwrap();
  assert!(
    kill(u32::MAX)
      .unwrap_err()
      .contains("process group 4294967295")
  );
}

#[test]
fn drain_keeps_everything_that_arrived_and_collect_returns_it() {
  let handle = super::drain(std::io::Cursor::new(b"first\nsecond\n".to_vec()));
  assert_eq!(super::collect(handle).unwrap(), "first\nsecond\n");
}

#[test]
fn a_reader_that_panicked_is_an_error() {
  let handle = std::thread::spawn(|| -> String { std::panic::resume_unwind(Box::new("boom")) });
  let err = super::collect(handle).unwrap_err();
  assert_eq!(err, "an output reader panicked: boom");
}

fn busy() -> std::io::Error {
  std::io::Error::from_raw_os_error(nix::errno::Errno::ETXTBSY as i32)
}

#[test]
fn a_busy_executable_is_tried_again_until_it_starts() {
  let mut calls = 0;
  let result = retry_busy(|| {
    calls += 1;
    if calls < 3 { Err(busy()) } else { Ok(calls) }
  });
  assert_eq!(result.unwrap(), 3);
}

#[test]
fn another_error_is_returned_at_once() {
  let mut calls = 0;
  let result: std::io::Result<()> = retry_busy(|| {
    calls += 1;
    Err(std::io::Error::from_raw_os_error(
      nix::errno::Errno::ENOENT as i32,
    ))
  });
  assert_eq!(calls, 1);
  assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn a_program_that_stays_busy_fails_after_the_retries() {
  let mut calls = 0;
  let result: std::io::Result<()> = retry_busy(|| {
    calls += 1;
    Err(busy())
  });
  assert_eq!(calls, 21);
  assert_eq!(result.unwrap_err().raw_os_error(), busy().raw_os_error());
}

#[test]
fn spawn_retrying_starts_a_program_and_reports_a_missing_one() {
  let mut ok = Command::new("/bin/sh");
  ok.args(["-c", "exit 0"]);
  assert!(spawn_retrying(&mut ok).unwrap().wait().unwrap().success());
  let mut missing = Command::new("/no/such/program");
  assert_eq!(
    spawn_retrying(&mut missing).unwrap_err().kind(),
    std::io::ErrorKind::NotFound
  );
}

/// Whether `pid` is gone within two seconds (init reaps a killed orphan shortly after).
fn gone(pid: i32) -> bool {
  let until = Instant::now() + Duration::from_secs(2);
  while Instant::now() < until {
    if nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_err() {
      return true;
    }
    std::thread::sleep(Duration::from_millis(20));
  }
  false
}

#[test]
fn stragglers_of_a_leader_that_exited_are_killed() {
  let dir = tempfile::tempdir().unwrap();
  let pid_file = dir.path().join("pid");
  let mut child = leader(&format!("sleep 30 & echo $! > '{}'", pid_file.display()));
  let status = wait(&mut child, Duration::from_secs(5), &mut || {}).unwrap();
  assert!(status.unwrap().success());
  let pid: i32 = std::fs::read_to_string(&pid_file)
    .unwrap()
    .trim()
    .parse()
    .unwrap();
  assert!(!gone(pid), "the straggler should still run before the kill");
  kill_stragglers(child.id()).unwrap();
  assert!(gone(pid), "straggler {pid} survived");
  kill_stragglers(child.id()).unwrap();
}
