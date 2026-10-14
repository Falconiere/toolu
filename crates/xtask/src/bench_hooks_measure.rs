//! One measured spawn for `cargo xtask bench-hooks`. Each sample is a fresh
//! `xtask measure` process: `RUSAGE_CHILDREN`'s max RSS is a high-water mark
//! over every child a process reaped, so one measurer per sample keeps a
//! light hook from inheriting a heavy one's reading.

use std::ffi::OsString;
use std::io::Write as _;
use std::os::unix::process::CommandExt as _;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::Duration;

use serde_json::Value;

use crate::bench_hooks_report::Sample;
use crate::process_group::{collect, drain, kill, wait};

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

/// Start the measurer as the leader of its own process group.
fn start(measurer: &Path, spawn: &Spawn<'_>, out: &Path, label: &str) -> Result<Child, String> {
  let mut child = Command::new(measurer)
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
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| format!("{label}: cannot start the measurer: {err}"))?;
  if let Some(mut stdin) = child.stdin.take() {
    // A hook that fails closed may exit before it reads.
    let _ignored = stdin.write_all(spawn.stdin.as_bytes());
  }
  Ok(child)
}

/// Wait for the measurer within the deadline, then kill what is left of its group
/// (a hook's background child could hold the stderr pipe).
fn finish(
  child: &mut Child,
  spawn: &Spawn<'_>,
  label: &str,
) -> Result<(Option<ExitStatus>, String), String> {
  let stderr = child.stderr.take().ok_or("no stderr pipe")?;
  let reader = drain(stderr);
  let waited = wait(child, spawn.deadline, &mut || {}).map_err(|err| format!("{label}: {err}"));
  let killed = kill(child.id());
  let stderr = collect(reader)?;
  let status = waited?;
  killed?;
  Ok((status, stderr))
}

fn once(measurer: &Path, spawn: &Spawn<'_>, label: &str, round: usize) -> Result<Sample, String> {
  let out = spawn.report_dir.join(format!("measure-{round}.json"));
  let mut child = start(measurer, spawn, &out, label)?;
  let (status, stderr) = finish(&mut child, spawn, label)?;
  let text = std::fs::read_to_string(&out);
  let _gone = std::fs::remove_file(&out);
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
