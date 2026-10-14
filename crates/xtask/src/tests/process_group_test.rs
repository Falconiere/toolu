use std::os::unix::process::CommandExt as _;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use super::{kill, wait};

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
