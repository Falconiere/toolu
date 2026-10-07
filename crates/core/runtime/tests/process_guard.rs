//! The runner's signal guard, black-box: a child process of this test guards a
//! `sleep 60` group it runs into a file. SIGTERM, SIGINT or SIGHUP to that
//! child kills the group and ends the child by the same signal, as a signalled
//! TypeScript ledger runner does (`guardGroup` in `ledger-check.ts`). This
//! binary has no libtest harness: run plainly it is the parent, and with the
//! arguments `child <dir>` it is the guarded runner.
//!
//! `main` is not test code to clippy, so it unwraps nothing and returns errors.

use std::io::Write as _;
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use toolu_runtime::process::file::{FileSpec, run_to_file};
use toolu_runtime::process::group;
use toolu_runtime::process::guard::GroupGuard;

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let result = match (args.first().map(String::as_str), args.get(1)) {
    (Some("child"), Some(dir)) => child(Path::new(dir)),
    _ => parent(),
  };
  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(err) => {
      std::io::stderr()
        .write_all(format!("process_guard: {err}\n").as_bytes())
        .ok();
      ExitCode::FAILURE
    }
  }
}

/// Guards a `sleep 60` group and reports its id in `<dir>/group`.
fn child(dir: &Path) -> Result<(), String> {
  let guard = GroupGuard::install()?;
  let spec = FileSpec {
    argv: vec!["sleep".to_owned(), "60".to_owned()],
    cwd: None,
    env: None,
    out: dir.join("out.log"),
    timeout: None,
    grace: Duration::from_secs(2),
  };
  let report = dir.join("group");
  run_to_file(&spec, &mut |pid| {
    guard.arm(pid);
    let _written = std::fs::write(&report, pid.to_string());
  })
  .map_err(|err| format!("{err:?}"))?;
  Err("the guarded sleep ended without the runner being signalled".to_owned())
}

fn wait_for(path: &Path) -> Result<u32, String> {
  let deadline = Instant::now() + Duration::from_secs(10);
  while Instant::now() < deadline {
    if let Ok(text) = std::fs::read_to_string(path)
      && let Ok(group) = text.trim().parse::<u32>()
    {
      return Ok(group);
    }
    std::thread::sleep(Duration::from_millis(20));
  }
  Err(format!("no group id at {}", path.display()))
}

fn dead_within(group: u32, limit: Duration) -> bool {
  let deadline = Instant::now() + limit;
  while Instant::now() < deadline {
    if !group::alive(group) {
      return true;
    }
    std::thread::sleep(Duration::from_millis(20));
  }
  !group::alive(group)
}

fn one(signal: Signal) -> Result<(), String> {
  let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
  let exe: PathBuf = std::env::current_exe().map_err(|err| err.to_string())?;
  let mut runner = Command::new(exe)
    .arg("child")
    .arg(dir.path())
    .spawn()
    .map_err(|err| err.to_string())?;
  let group = wait_for(&dir.path().join("group"))?;
  let pid = i32::try_from(runner.id()).map_err(|err| err.to_string())?;
  kill(Pid::from_raw(pid), signal).map_err(|err| err.to_string())?;
  let status = runner.wait().map_err(|err| err.to_string())?;
  if status.signal() != Some(signal as i32) {
    return Err(format!("{signal}: the runner ended with {status:?}"));
  }
  if !dead_within(group, Duration::from_secs(2)) {
    let _killed = group::signal(group, Signal::SIGKILL);
    return Err(format!(
      "{signal}: the check's group {group} outlived its runner"
    ));
  }
  Ok(())
}

fn parent() -> Result<(), String> {
  for signal in [Signal::SIGTERM, Signal::SIGINT, Signal::SIGHUP] {
    one(signal)?;
  }
  Ok(())
}
