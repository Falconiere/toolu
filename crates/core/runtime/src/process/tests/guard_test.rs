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

#[test]
fn a_child_of_a_guarded_thread_starts_with_the_signals_unblocked() {
  use nix::sys::signal::{SigSet, SigmaskHow, pthread_sigmask};
  let _guard = GroupGuard::install().unwrap();
  // Whichever thread installed the guard, this one now blocks the signals too.
  let mut signals = SigSet::empty();
  for signal in [
    nix::sys::signal::Signal::SIGINT,
    nix::sys::signal::Signal::SIGTERM,
    nix::sys::signal::Signal::SIGHUP,
  ] {
    signals.add(signal);
  }
  pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&signals), None).unwrap();
  let piped = crate::process::run(&crate::process::Spec::new([
    "bash",
    "-c",
    "kill -TERM $$; echo survived",
  ]))
  .unwrap();
  assert_eq!((piped.exit_code, piped.stdout.as_str()), (128 + 15, ""));
  let dir = tempfile::tempdir().unwrap();
  let spec = crate::process::file::FileSpec {
    argv: vec![
      "bash".to_owned(),
      "-c".to_owned(),
      "kill -TERM $$; echo survived".to_owned(),
    ],
    cwd: None,
    env: None,
    out: dir.path().join("out"),
    timeout: None,
    grace: Duration::from_secs(1),
  };
  let filed = crate::process::file::run_to_file(&spec, &mut |_| {}).unwrap();
  assert_eq!(filed.exit_code, 128 + 15);
  pthread_sigmask(SigmaskHow::SIG_UNBLOCK, Some(&signals), None).unwrap();
}
