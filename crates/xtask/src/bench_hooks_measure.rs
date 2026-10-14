//! One measured spawn for `cargo xtask bench-hooks`. Each sample is a fresh
//! `xtask measure` process: `RUSAGE_CHILDREN`'s max RSS is a high-water mark
//! over every child a process reaped, so one measurer per sample keeps a
//! light hook from inheriting a heavy one's reading.

use std::ffi::OsString;
use std::fs::File;
use std::io::Write as _;
use std::os::unix::process::CommandExt as _;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::Value;

use crate::bench_hooks_report::Sample;
use crate::process_group::{kill_stragglers, spawn_retrying, wait};

/// A measurer that runs longer than this is killed with its whole process group.
pub(crate) const SPAWN_DEADLINE: Duration = Duration::from_secs(60);

/// What to spawn and how.
pub(crate) struct Spawn<'s> {
  pub(crate) argv: Vec<String>,
  pub(crate) cwd: &'s Path,
  pub(crate) env: Vec<(OsString, OsString)>,
  pub(crate) stdin: String,
  /// A private directory the measurer writes its report in.
  pub(crate) report_dir: &'s Path,
  /// How long one spawn may take.
  pub(crate) deadline: Duration,
}

/// `warmup + runs` measured spawns one after another; the last `runs` are kept.
pub(crate) fn samples(
  measurer: &Path,
  spawn: &Spawn<'_>,
  label: &str,
  (warmup, runs): (usize, usize),
) -> Result<Vec<Sample>, String> {
  let mut kept = Vec::new();
  for round in 0..warmup + runs {
    let sample = once(measurer, spawn, label, round)?;
    if round >= warmup {
      kept.push(sample);
    }
  }
  Ok(kept)
}

/// Write `payload` to the measurer's stdin on a thread, so a measurer that never reads
/// cannot stall the deadline; the pipe closing when it dies ends the write.
fn feed(mut stdin: ChildStdin, payload: Vec<u8>) -> JoinHandle<()> {
  std::thread::spawn(move || {
    // A hook that fails closed may exit before it reads.
    let _ignored = stdin.write_all(&payload);
  })
}

/// Start the measurer as the leader of its own process group, its stderr in `err`.
/// A file, not a pipe, so a hook's background child cannot hold the harness waiting.
fn start(
  measurer: &Path,
  spawn: &Spawn<'_>,
  (out, err): (&Path, &Path),
  label: &str,
) -> Result<(Child, JoinHandle<()>), String> {
  let stderr = File::create(err).map_err(|why| format!("{label}: {}: {why}", err.display()))?;
  let mut command = Command::new(measurer);
  command
    .args(["measure", "--out"])
    .arg(out)
    .arg("--")
    .args(&spawn.argv)
    .current_dir(spawn.cwd)
    .env_clear()
    .envs(spawn.env.iter().map(|(key, value)| (key, value)))
    .process_group(0)
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::from(stderr));
  let mut child = spawn_retrying(&mut command)
    .map_err(|why| format!("{label}: cannot start the measurer: {why}"))?;
  let stdin = child.stdin.take().ok_or("no stdin pipe")?;
  Ok((child, feed(stdin, spawn.stdin.clone().into_bytes())))
}

/// Wait for the measurer within the deadline; past it, `wait` kills its whole group
/// before reaping the leader. After a normal exit a hook's background children are
/// killed too, so none runs on into the next sample.
fn finish(
  (mut child, writer): (Child, JoinHandle<()>),
  spawn: &Spawn<'_>,
  label: &str,
) -> Result<Option<ExitStatus>, String> {
  let waited =
    wait(&mut child, spawn.deadline, &mut || {}).map_err(|err| format!("{label}: {err}"));
  if matches!(waited, Ok(Some(_))) {
    kill_stragglers(child.id()).map_err(|err| format!("{label}: {err}"))?;
  }
  // A writer still blocked now is held by a hook's background child that kept the pipe
  // open; it ends with that child, and waiting for it would defeat the deadline.
  if writer.is_finished() {
    writer
      .join()
      .map_err(|_panic| format!("{label}: the stdin writer panicked"))?;
  }
  waited
}

fn once(measurer: &Path, spawn: &Spawn<'_>, label: &str, round: usize) -> Result<Sample, String> {
  let out = spawn.report_dir.join(format!("measure-{round}.json"));
  let err = out.with_extension("err");
  let started = start(measurer, spawn, (&out, &err), label)?;
  let status = finish(started, spawn, label)?;
  let text = std::fs::read_to_string(&out);
  let stderr =
    std::fs::read_to_string(&err).unwrap_or_else(|why| format!("(stderr unreadable: {why})"));
  let _gone = (std::fs::remove_file(&out), std::fs::remove_file(&err));
  let Some(status) = status else {
    return Err(format!(
      "{label}: the measurer ran past {} s and was killed: {}",
      spawn.deadline.as_secs_f64(),
      stderr.trim()
    ));
  };
  if !status.success() {
    return Err(format!(
      "{label}: measurer exited {:?}: {}",
      status.code(),
      stderr.trim()
    ));
  }
  let report: Value = serde_json::from_str(&text.map_err(|err| format!("{label}: {err}"))?)
    .map_err(|err| format!("{label}: report: {err}"))?;
  sample_of(&report).map_err(|err| format!("{label}: {err}"))
}

/// A report whose hook exited 0, reduced to a sample; a broken hook must not look cheap.
pub(crate) fn sample_of(report: &Value) -> Result<Sample, String> {
  let field = |key: &str| {
    report
      .get(key)
      .and_then(Value::as_u64)
      .ok_or(format!("report lacks {key}"))
  };
  if report.get("exitCode").and_then(Value::as_i64) != Some(0) {
    return Err(format!(
      "the hook ended with exit {} signal {}",
      report.get("exitCode").unwrap_or(&Value::Null),
      report.get("signal").unwrap_or(&Value::Null)
    ));
  }
  Ok(Sample {
    max_rss_bytes: field("maxRssBytes")?,
    cpu_us: field("userUs")? + field("sysUs")?,
    wall_us: field("wallUs")?,
  })
}

#[cfg(test)]
#[path = "tests/bench_hooks_measure_test.rs"]
mod tests;
