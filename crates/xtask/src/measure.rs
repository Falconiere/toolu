//! `cargo xtask measure --out FILE -- COMMAND…`: run one command and report the
//! resources it used, for the hook resource benchmark (#410).
//!
//! The measurer must stay small: Linux folds the old image's RSS high-water mark
//! into a child's `maxrss` at `exec`, so a child spawned by a large process (Bun)
//! would report that process's RSS. With exactly one child, `RUSAGE_CHILDREN` is
//! that child's tree: the command plus every descendant it waited for.

use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use nix::sys::resource::{UsageWho, getrusage};
use nix::sys::time::TimeVal;
use serde::Serialize;

use crate::Verdict;
use crate::options::Options;

/// The report format version.
const VERSION: u32 = 1;

/// What one run of a command used.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Report {
  /// The report format version.
  version: u32,
  /// The command as run.
  command: Vec<String>,
  /// Its exit code; null when a signal ended it.
  exit_code: Option<i32>,
  /// The signal that ended it, if any.
  signal: Option<i32>,
  /// Wall time from spawn to reap, in microseconds.
  wall_us: u64,
  /// User CPU time of the command and its descendants, in microseconds.
  user_us: u64,
  /// System CPU time of the command and its descendants, in microseconds.
  sys_us: u64,
  /// The largest resident set of any process in the tree, in bytes.
  max_rss_bytes: u64,
}

/// Measure the command after `--` and write its report to `--out`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let out = options.out.as_deref().ok_or("measure needs --out FILE")?;
  let report = measure(&options.command)?;
  write(out, &report)?;
  Ok(Verdict::Clean)
}

/// Spawn `command` once with inherited stdio and wait for it.
pub(crate) fn measure(command: &[String]) -> Result<Report, String> {
  let (program, args) = command
    .split_first()
    .ok_or("measure needs a command after --")?;
  let started = Instant::now();
  let status = Command::new(program)
    .args(args)
    .status()
    .map_err(|err| format!("measure: cannot run {program}: {err}"))?;
  let wall = started.elapsed();
  let usage =
    getrusage(UsageWho::RUSAGE_CHILDREN).map_err(|err| format!("measure: getrusage: {err}"))?;
  Ok(Report {
    version: VERSION,
    command: command.to_vec(),
    exit_code: status.code(),
    signal: status.signal(),
    wall_us: unsigned("wall time", wall.as_micros())?,
    user_us: micros("user time", usage.user_time())?,
    sys_us: micros("system time", usage.system_time())?,
    max_rss_bytes: rss_bytes(unsigned("ru_maxrss", usage.max_rss())?),
  })
}

/// `value` as a `u64`; a negative or oversized reading fails the measurement
/// rather than being reported as a made-up number.
pub(crate) fn unsigned<T>(what: &str, value: T) -> Result<u64, String>
where
  T: TryInto<u64> + Copy + std::fmt::Display,
  T::Error: std::fmt::Display,
{
  value
    .try_into()
    .map_err(|err| format!("measure: {what} out of range: {value} ({err})"))
}

/// `time` in whole microseconds.
pub(crate) fn micros(what: &str, time: TimeVal) -> Result<u64, String> {
  let seconds = unsigned(what, time.tv_sec())?;
  let micros = unsigned(what, time.tv_usec())?;
  seconds
    .checked_mul(1_000_000)
    .and_then(|us| us.checked_add(micros))
    .ok_or_else(|| format!("measure: {what} out of range: {time}"))
}

/// `ru_maxrss` in bytes: Apple kernels report bytes, the others KiB.
pub(crate) fn rss_bytes(raw: u64) -> u64 {
  if cfg!(target_vendor = "apple") {
    raw
  } else {
    raw.saturating_mul(1024)
  }
}

/// Write `report` as one JSON line to `out`.
fn write(out: &Path, report: &Report) -> Result<(), String> {
  let text = serde_json::to_string(report).map_err(|err| format!("measure: {err}"))?;
  std::fs::write(out, format!("{text}\n"))
    .map_err(|err| format!("measure: cannot write {}: {err}", out.display()))
}

#[cfg(test)]
#[path = "tests/measure_test.rs"]
mod tests;
