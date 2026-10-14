//! Run one child process of `host-smoke` with a deadline, its output captured, and an
//! optional callback that fires every 20 ms while it runs (the Codex edit poller).

use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread::{JoinHandle, sleep, spawn};
use std::time::{Duration, Instant};

use crate::host_smoke_env::ChildEnv;

/// Each live session gets this long before it is killed.
pub(crate) const TIMEOUT: Duration = Duration::from_secs(180);
const POLL: Duration = Duration::from_millis(20);
const TAIL: usize = 800;

/// A child that ran to the end.
#[derive(Debug)]
pub(crate) struct Finished {
  pub(crate) success: bool,
  pub(crate) code: Option<i32>,
  pub(crate) output: String,
}

fn drain<R: Read + Send + 'static>(mut stream: R) -> JoinHandle<String> {
  spawn(move || {
    let mut bytes = Vec::new();
    // A read that fails mid-stream keeps what arrived: the transcript is evidence.
    stream.read_to_end(&mut bytes).ok();
    String::from_utf8_lossy(&bytes).into_owned()
  })
}

fn wait(
  child: &mut std::process::Child,
  timeout: Duration,
  tick: &mut dyn FnMut(),
) -> Result<ExitStatus, String> {
  let deadline = Instant::now() + timeout;
  loop {
    tick();
    if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
      tick();
      return Ok(status);
    }
    if Instant::now() >= deadline {
      child.kill().map_err(|err| err.to_string())?;
      child.wait().map_err(|err| err.to_string())?;
      return Err(format!("timed out after {} s", timeout.as_secs()));
    }
    sleep(POLL);
  }
}

fn collect(handle: JoinHandle<String>) -> Result<String, String> {
  handle.join().map_err(|payload| {
    let why = payload
      .downcast_ref::<&str>()
      .copied()
      .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
      .unwrap_or("unknown cause");
    format!("an output reader panicked: {why}")
  })
}

/// Run `argv` in `cwd` with `env` and no stdin; `tick` runs while it does.
pub(crate) fn exec(
  argv: &[String],
  cwd: &Path,
  env: &ChildEnv,
  timeout: Duration,
  tick: &mut dyn FnMut(),
) -> Result<Finished, String> {
  let (program, rest) = argv.split_first().ok_or("empty argv")?;
  let mut command = Command::new(program);
  command
    .args(rest)
    .current_dir(cwd)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  env.apply(&mut command);
  let mut child = command
    .spawn()
    .map_err(|err| format!("cannot run {program}: {err}"))?;
  let out = child.stdout.take().ok_or("no stdout pipe")?;
  let err = child.stderr.take().ok_or("no stderr pipe")?;
  let (out, err) = (drain(out), drain(err));
  let status = wait(&mut child, timeout, tick).map_err(|why| format!("{program} {why}"))?;
  let output = format!("{}{}", collect(out)?, collect(err)?);
  Ok(Finished {
    success: status.success(),
    code: status.code(),
    output,
  })
}

/// The last `TAIL` characters of `text`.
pub(crate) fn tail(text: &str) -> String {
  let skip = text.chars().count().saturating_sub(TAIL);
  text.chars().skip(skip).collect()
}

/// Run `argv`, write its output to `log`, and return the output when it exits 0.
pub(crate) fn run(
  argv: &[String],
  cwd: &Path,
  env: &ChildEnv,
  log: Option<&Path>,
) -> Result<String, String> {
  let done = exec(argv, cwd, env, TIMEOUT, &mut || {})?;
  if let Some(log) = log {
    std::fs::write(log, &done.output)
      .map_err(|err| format!("cannot write {}: {err}", log.display()))?;
  }
  if done.success {
    return Ok(done.output);
  }
  let program = argv.first().map_or("", String::as_str);
  Err(format!(
    "{program} exited {:?}: {}",
    done.code,
    tail(&done.output)
  ))
}

#[cfg(test)]
#[path = "tests/host_smoke_exec_test.rs"]
mod tests;
