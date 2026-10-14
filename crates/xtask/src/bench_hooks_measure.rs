//! One measured spawn for `cargo xtask bench-hooks`. Each sample is a fresh
//! `xtask measure` process: `RUSAGE_CHILDREN`'s max RSS is a high-water mark
//! over every child a process reaped, so one measurer per sample keeps a
//! light hook from inheriting a heavy one's reading.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::bench_hooks_report::Sample;

/// What to spawn and how.
pub(crate) struct Spawn<'s> {
  pub(crate) argv: Vec<String>,
  pub(crate) cwd: &'s Path,
  pub(crate) env: Vec<(String, String)>,
  pub(crate) stdin: String,
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

fn once(measurer: &Path, spawn: &Spawn<'_>, label: &str, round: usize) -> Result<Sample, String> {
  let out = report_path(round);
  let mut child = Command::new(measurer)
    .args(["measure", "--out"])
    .arg(&out)
    .arg("--")
    .args(&spawn.argv)
    .current_dir(spawn.cwd)
    .env_clear()
    .envs(spawn.env.iter().map(|(key, value)| (key, value)))
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| format!("{label}: cannot start the measurer: {err}"))?;
  if let Some(mut stdin) = child.stdin.take() {
    // A hook that fails closed may exit before it reads.
    let _ignored = stdin.write_all(spawn.stdin.as_bytes());
  }
  let output = child
    .wait_with_output()
    .map_err(|err| format!("{label}: {err}"))?;
  let text = std::fs::read_to_string(&out);
  let _gone = std::fs::remove_file(&out);
  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(format!(
      "{label}: measurer exited {:?}: {}",
      output.status.code(),
      stderr.trim()
    ));
  }
  let report: Value = serde_json::from_str(&text.map_err(|err| format!("{label}: {err}"))?)
    .map_err(|err| format!("{label}: report: {err}"))?;
  sample_of(&report).map_err(|err| format!("{label}: {err}"))
}

fn report_path(round: usize) -> PathBuf {
  std::env::temp_dir().join(format!(
    "toolu-hook-bench-{}-{round}.json",
    std::process::id()
  ))
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
