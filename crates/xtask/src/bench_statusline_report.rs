//! The `toolu.statusline-benchmark/v1` result of `cargo xtask bench-statusline`, its
//! fixed #431 budget and the verdict against it.

use std::process::Command;

use serde::Serialize;

use crate::bench_hooks_report::{MIB, Sample};

/// Unmeasured renders before the measured ones.
pub(crate) const WARMUP: usize = 3;

/// The p50 wall budget of one render, in microseconds.
pub(crate) const WALL_US: u64 = 4000;

/// The p50 max RSS budget of one render, in bytes. 5 MiB until CI hosts measured the
/// unchanged render at up to 5.1 MiB (`benchmarks/results/statusline-rust-linux-ci-2026-10-15.json`).
pub(crate) const RSS_BYTES: u64 = 6 * MIB;

/// What one bench run measured: warm p50s and the cold first render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Measured {
  pub(crate) runs: usize,
  pub(crate) wall_us: u64,
  pub(crate) max_rss_bytes: u64,
  pub(crate) cold: Sample,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Cold {
  wall_us: u64,
  max_rss_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Budget {
  wall_us: u64,
  max_rss_bytes: u64,
}

/// The result document; field order is the committed files' order.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Document {
  schema: &'static str,
  platform: &'static str,
  cpu: String,
  runs: usize,
  warmup: usize,
  wall_us: u64,
  max_rss_bytes: u64,
  cold_first_render: Cold,
  budget: Budget,
}

/// The result of `measured` on `cpu`.
pub(crate) fn document(measured: &Measured, cpu: String) -> Document {
  Document {
    schema: "toolu.statusline-benchmark/v1",
    platform: platform(),
    cpu,
    runs: measured.runs,
    warmup: WARMUP,
    wall_us: measured.wall_us,
    max_rss_bytes: measured.max_rss_bytes,
    cold_first_render: Cold {
      wall_us: measured.cold.wall_us,
      max_rss_bytes: measured.cold.max_rss_bytes,
    },
    budget: Budget {
      wall_us: WALL_US,
      max_rss_bytes: RSS_BYTES,
    },
  }
}

/// One line per budget the warm p50 exceeds.
pub(crate) fn violations(measured: &Measured) -> Vec<String> {
  let mut found = Vec::new();
  if measured.wall_us > WALL_US {
    found.push(format!(
      "statusline render: wall p50 {} us > budget {WALL_US} us",
      measured.wall_us
    ));
  }
  if measured.max_rss_bytes > RSS_BYTES {
    found.push(format!(
      "statusline render: rss p50 {} bytes > budget {RSS_BYTES} bytes",
      measured.max_rss_bytes
    ));
  }
  found
}

/// The OS name Node's `os.platform()` gave the committed results.
fn platform() -> &'static str {
  if std::env::consts::OS == "macos" {
    "darwin"
  } else {
    std::env::consts::OS
  }
}

/// The first CPU's model name, or `unknown`.
pub(crate) fn cpu_model() -> String {
  let sysctl = Command::new("sysctl")
    .args(["-n", "machdep.cpu.brand_string"])
    .output()
    .ok()
    .filter(|out| out.status.success())
    .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned());
  let cpuinfo = || {
    let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    let line = text.lines().find(|line| line.starts_with("model name"))?;
    line
      .split_once(':')
      .map(|(_, model)| model.trim().to_owned())
  };
  sysctl
    .or_else(cpuinfo)
    .filter(|model| !model.is_empty())
    .unwrap_or_else(|| "unknown".to_owned())
}

#[cfg(test)]
#[path = "tests/bench_statusline_report_test.rs"]
mod tests;
