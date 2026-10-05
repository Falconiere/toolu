//! `cargo xtask check-startup --bin FILE`: the `toolu --version` startup budget
//! of `docs/resource-budgets.md` (#442, #410). It spawns the binary the budget's
//! warm-up and run counts, one at a time, and compares the nearest-rank
//! percentile of the wall times (the hook bench's method) with the budget in
//! `benchmarks/startup-budgets.json`. The Linux `rust` CI job runs it on the
//! release binary; macOS is not gated.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::options::Options;
use crate::{Verdict, output};

/// The budget file, relative to the repository root.
pub(crate) const BUDGET: &str = "benchmarks/startup-budgets.json";

/// `benchmarks/startup-budgets.json`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Budget {
  /// The format version: 1.
  version: u32,
  /// The arguments `toolu` runs with.
  pub(crate) command: Vec<String>,
  /// Unmeasured spawns first.
  warmup: usize,
  /// Measured spawns.
  runs: usize,
  /// The percentile judged (nearest rank).
  pub(crate) percentile: usize,
  /// The budget for that percentile, in milliseconds.
  pub(crate) wall_ms: u64,
}

/// Measure `--bin` and judge it against the budget.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let bin = options
    .bin
    .as_deref()
    .ok_or("check-startup needs --bin FILE")?;
  let budget = load(&options.root)?;
  for _ in 0..budget.warmup {
    spawn(bin, &budget.command)?;
  }
  let walls = (0..budget.runs)
    .map(|_| spawn(bin, &budget.command))
    .collect::<Result<Vec<_>, _>>()?;
  match judge(&walls, &budget) {
    Ok(report) => {
      output::say(&report);
      Ok(Verdict::Clean)
    }
    Err(report) => {
      output::error(&report);
      Ok(Verdict::Findings)
    }
  }
}

/// Read and check the budget file under `root`.
pub(crate) fn load(root: &Path) -> Result<Budget, String> {
  let text = std::fs::read_to_string(root.join(BUDGET))
    .map_err(|err| format!("cannot read {BUDGET}: {err}"))?;
  let budget: Budget =
    serde_json::from_str(&text).map_err(|err| format!("{BUDGET} is malformed: {err}"))?;
  if budget.version != 1
    || budget.runs == 0
    || budget.command.is_empty()
    || !(1..=100).contains(&budget.percentile)
  {
    return Err(format!(
      "{BUDGET} needs version 1, a command, at least one run and a percentile of 1 to 100"
    ));
  }
  Ok(budget)
}

/// One spawn of `bin args`: its wall time, if it printed `toolu <version>` and exited 0.
fn spawn(bin: &Path, args: &[String]) -> Result<Duration, String> {
  let shown = format!("{} {}", bin.display(), args.join(" "));
  let started = Instant::now();
  let ran = Command::new(bin)
    .args(args)
    .stdin(Stdio::null())
    .stderr(Stdio::null())
    .output()
    .map_err(|err| format!("cannot run {shown}: {err}"))?;
  let wall = started.elapsed();
  if !ran.status.success() {
    return Err(format!("{shown} exited {}", ran.status));
  }
  let stdout = String::from_utf8_lossy(&ran.stdout);
  if !stdout.starts_with("toolu ") {
    return Err(format!(
      "{shown} printed {:?}, not `toolu <version>`",
      stdout.trim_end()
    ));
  }
  Ok(wall)
}

/// The report for `walls`: `Ok` within the budget, `Err` over it.
pub(crate) fn judge(walls: &[Duration], budget: &Budget) -> Result<String, String> {
  let mut sorted = walls.to_vec();
  sorted.sort();
  let at = |percentile: usize| {
    let rank = (percentile * sorted.len()).div_ceil(100).max(1);
    sorted.get(rank - 1).copied().unwrap_or_default()
  };
  let (judged, p90) = (at(budget.percentile), at(90));
  let limit = Duration::from_millis(budget.wall_ms);
  let report = format!(
    "check-startup: toolu {} wall p{} {:.2} ms, p90 {:.2} ms over {} runs (budget {} ms)",
    budget.command.join(" "),
    budget.percentile,
    judged.as_secs_f64() * 1000.0,
    p90.as_secs_f64() * 1000.0,
    sorted.len(),
    budget.wall_ms
  );
  if judged <= limit {
    Ok(report)
  } else {
    Err(format!("{report}: over budget"))
  }
}

#[cfg(test)]
#[path = "tests/startup_test.rs"]
mod tests;
