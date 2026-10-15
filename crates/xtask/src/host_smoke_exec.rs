//! Run one child process of `host-smoke` with a deadline, its output captured, and an
//! optional callback that fires every 20 ms while it runs (the Codex edit poller).

use std::os::unix::process::CommandExt as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::host_smoke_env::ChildEnv;
use crate::process_group::{collect, drain, kill_stragglers, spawn_retrying, wait};

/// Each live session gets this long before it is killed.
pub(crate) const TIMEOUT: Duration = Duration::from_secs(180);
const TAIL: usize = 800;

/// A child that ran to the end.
#[derive(Debug)]
pub(crate) struct Finished {
  pub(crate) success: bool,
  pub(crate) code: Option<i32>,
  pub(crate) output: String,
}

/// Run `argv` in `cwd` with `env` and no stdin; `tick` runs while it does. The child
/// leads its own process group, killed when it times out and again after it exits, so a
/// grandchild never holds the output pipes open. A timeout keeps what was printed.
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
    .process_group(0)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  env.apply(&mut command);
  let mut child =
    spawn_retrying(&mut command).map_err(|err| format!("cannot run {program}: {err}"))?;
  let out = child.stdout.take().ok_or("no stdout pipe")?;
  let err = child.stderr.take().ok_or("no stderr pipe")?;
  let (out, err) = (drain(out), drain(err));
  let waited = wait(&mut child, timeout, tick).map_err(|why| format!("{program} {why}"));
  // A timeout was killed and reaped by `wait` itself; after a normal exit the stragglers
  // holding the pipes go too.
  let killed = match &waited {
    Ok(Some(_)) => kill_stragglers(child.id()),
    Ok(None) | Err(_) => Ok(()),
  };
  let output = format!("{}{}", collect(out)?, collect(err)?);
  let Some(status) = waited? else {
    return Err(format!(
      "{program} timed out after {} s; output so far: {}",
      timeout.as_secs(),
      tail(&output)
    ));
  };
  killed?;
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
