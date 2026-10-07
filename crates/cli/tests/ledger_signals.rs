//! A signal mid-check (#421, AC-7): SIGTERM, SIGINT or SIGHUP to
//! `toolu ledger run` kills the check's whole process group within two
//! seconds, then ends `toolu` by the same signal, as the TypeScript CLI's
//! `guardGroup` does; the interrupted step is never stamped green.

#[path = "helpers/ledger.rs"]
mod ledger;

use std::os::unix::process::ExitStatusExt as _;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use ledger::{Project, Res};
use serde_json::Value;

/// Polls `done` every 20 ms until it answers or `limit` passes.
fn within<T>(limit: Duration, mut done: impl FnMut() -> Res<Option<T>>) -> Res<T> {
  let deadline = Instant::now() + limit;
  loop {
    if let Some(value) = done()? {
      return Ok(value);
    }
    if Instant::now() > deadline {
      return Err(format!("nothing within {limit:?}").into());
    }
    std::thread::sleep(Duration::from_millis(20));
  }
}

/// Whether `pid` is gone or a zombie waiting for its reaper.
fn dead(project: &Project, pid: &str) -> Res<bool> {
  let state = project.sh(&format!("ps -o stat= -p {pid} || true"))?;
  Ok(state.trim().is_empty() || state.trim().starts_with('Z'))
}

/// `toolu ledger run plan.md --step s1` over a check that backgrounds
/// `sleep 60` and waits; returns the runner and the sleeper's pid.
fn running(project: &Project) -> Res<(Child, String)> {
  let pid_file = project.home.join("sleep.pid");
  let check = format!("sleep 60 & echo $! > '{}'; wait", pid_file.display());
  let steps = serde_json::json!([{ "id": "s1", "title": "sleeps", "check": check }]);
  project.write(
    "plan.md",
    &format!(
      "# P\n\n**Status:** Approved\n\n## Steps (machine-readable)\n\n```json\n{steps}\n```\n"
    ),
  )?;
  let child = project
    .command(&project.root)
    .args(["ledger", "run", "plan.md", "--step", "s1"])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()?;
  let pid = within(Duration::from_secs(30), || {
    Ok(
      std::fs::read_to_string(&pid_file)
        .ok()
        .filter(|pid| pid.ends_with('\n')),
    )
  })?;
  Ok((child, pid.trim().to_owned()))
}

/// Sends `signal` to the runner mid-check; returns the signal that ended it,
/// whether the sleeper died within two seconds, and the step's status.
fn interrupted(signal: &str) -> Res<(Option<i32>, bool, Value)> {
  let project = Project::new()?;
  let (mut child, sleeper) = running(&project)?;
  if dead(&project, &sleeper)? {
    return Err("the check never started".into());
  }
  let target = child.id().to_string();
  if !Command::new("kill")
    .args([&format!("-{signal}"), &target])
    .status()?
    .success()
  {
    return Err(format!("kill -{signal} failed").into());
  }
  let status = within(Duration::from_secs(5), || Ok(child.try_wait()?))?;
  let gone = within(Duration::from_secs(2), || {
    Ok(dead(&project, &sleeper)?.then_some(true))
  });
  let ledger: Value = serde_json::from_str(&std::fs::read_to_string(project.ledger())?)?;
  let step = ledger
    .pointer("/steps/0/status")
    .cloned()
    .unwrap_or(Value::Null);
  Ok((status.signal(), gone.is_ok(), step))
}

/// A signalled runner leaves its step `running` for the stuck check, as TypeScript does.
fn killed(signal: i32) -> (Option<i32>, bool, Value) {
  (Some(signal), true, Value::from("running"))
}

#[test]
fn sigterm_kills_the_check_group_and_ends_toolu() {
  assert_eq!(interrupted("TERM").unwrap(), killed(15));
}

#[test]
fn sigint_kills_the_check_group_and_ends_toolu() {
  assert_eq!(interrupted("INT").unwrap(), killed(2));
}

#[test]
fn sighup_kills_the_check_group_and_ends_toolu() {
  assert_eq!(interrupted("HUP").unwrap(), killed(1));
}
