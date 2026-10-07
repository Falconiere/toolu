//! Bounded machine sampling (`sampleResources` in `packages/toolu-core/src/resources/pressure.ts`).
//! Linux reads `/proc/stat` (logical CPUs, tick counters and steal), `/proc/meminfo`
//! (`MemTotal`, `MemAvailable`) and `/proc/loadavg`. macOS asks `sysctl` and
//! `vm_stat`, counting free, inactive and speculative pages as available.
//! The parsers are pure, so every platform tests them.

use super::pressure::{ResourceSample, Ticks};

/// `/proc/stat`: the logical CPUs (`cpuN` rows) and the first row's ticks.
pub fn parse_proc_stat(text: &str) -> (usize, Option<Ticks>) {
  let cpus = text
    .lines()
    .filter(|line| {
      line
        .strip_prefix("cpu")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
    })
    .count();
  let ticks = text.lines().next().map(|first| {
    let fields: Vec<f64> = first
      .split_whitespace()
      .skip(1)
      .map(|field| field.parse::<f64>().unwrap_or(f64::NAN))
      .collect();
    // guest fields are already counted in user and nice, so the total stops at steal.
    Ticks {
      total: fields.iter().take(8).sum(),
      steal: fields.get(7).copied().unwrap_or(0.0),
    }
  });
  (cpus, ticks)
}

/// `/proc/meminfo`'s `<key>: N kB` in bytes.
pub fn meminfo_bytes(text: &str, key: &str) -> Option<f64> {
  text.lines().find_map(|line| {
    let value = line.strip_prefix(key)?.strip_prefix(':')?.trim_start();
    let kb = value.strip_suffix(" kB")?;
    (!kb.is_empty() && kb.bytes().all(|b| b.is_ascii_digit()))
      .then(|| kb.parse::<f64>().ok().map(|kb| kb * 1024.0))
      .flatten()
  })
}

/// The one-minute load average of `/proc/loadavg` or `sysctl -n vm.loadavg`.
pub fn parse_load(text: &str) -> f64 {
  text
    .split_whitespace()
    .find_map(|word| word.parse::<f64>().ok())
    .unwrap_or(0.0)
}

/// `vmStatAvailableBytes`: free, inactive and speculative pages; purgeable
/// pages overlap active and inactive and stay uncounted.
pub fn vm_stat_available_bytes(text: &str) -> Option<f64> {
  let page_size = text
    .split("page size of ")
    .nth(1)?
    .split(" bytes")
    .next()?
    .parse::<f64>()
    .ok()?;
  let mut pages = 0.0;
  for kind in ["free", "inactive", "speculative"] {
    let prefix = format!("Pages {kind}:");
    let count = text.lines().find_map(|line| {
      let rest = line.strip_prefix(&prefix)?.trim_start().strip_suffix('.')?;
      rest.parse::<f64>().ok()
    })?;
    pages += count;
  }
  Some(pages * page_size)
}

/// The share of stolen ticks since `before`, when the counters moved forward.
pub fn steal_since(before: Option<Ticks>, now: Ticks) -> Option<f64> {
  let before = before?;
  (now.total > before.total)
    .then(|| ((now.steal - before.steal) / (now.total - before.total)).max(0.0))
}

/// `sampleResources(previous)` at `now_ms`.
pub fn sample_resources(previous: Option<&ResourceSample>, now_ms: f64) -> ResourceSample {
  platform(previous, now_ms)
}

#[cfg(target_os = "linux")]
fn platform(previous: Option<&ResourceSample>, now_ms: f64) -> ResourceSample {
  let read = |path: &str| std::fs::read_to_string(path).unwrap_or_default();
  let (stat, meminfo) = (read("/proc/stat"), read("/proc/meminfo"));
  let (cpus, ticks) = parse_proc_stat(&stat);
  let total_bytes = meminfo_bytes(&meminfo, "MemTotal").unwrap_or(0.0);
  let available_bytes = meminfo_bytes(&meminfo, "MemAvailable")
    .or_else(|| meminfo_bytes(&meminfo, "MemFree"))
    .unwrap_or(0.0);
  let steal = ticks.and_then(|now| steal_since(previous.and_then(|p| p.ticks), now));
  ResourceSample {
    at: now_ms,
    cpus: f64::from(u32::try_from(cpus).unwrap_or(0)),
    load: parse_load(&read("/proc/loadavg")),
    available_bytes,
    total_bytes,
    steal,
    ticks,
  }
}

#[cfg(not(target_os = "linux"))]
fn platform(_previous: Option<&ResourceSample>, now_ms: f64) -> ResourceSample {
  use std::time::Duration;
  use toolu_runtime::process::{Spec, run};
  let output = |argv: &[&str]| {
    let mut spec = Spec::new(argv.iter().copied());
    spec.timeout = Duration::from_secs(2);
    run(&spec)
      .ok()
      .filter(|output| output.exit_code == 0)
      .map(|output| output.stdout)
      .unwrap_or_default()
  };
  let sysctl = output(&[
    "sysctl",
    "-n",
    "hw.ncpu",
    "hw.memsize",
    "vm.loadavg",
    "vm.page_free_count",
    "hw.pagesize",
  ]);
  let lines: Vec<&str> = sysctl.lines().collect();
  let field = |at: usize| {
    lines
      .get(at)
      .and_then(|line| line.trim().parse::<f64>().ok())
      .unwrap_or(0.0)
  };
  let total_bytes = field(1);
  let free = field(3) * field(4);
  let available_bytes = vm_stat_available_bytes(&output(&["/usr/bin/vm_stat"]))
    .map_or(free, |bytes| bytes.min(total_bytes));
  ResourceSample {
    at: now_ms,
    cpus: field(0),
    load: parse_load(
      lines
        .get(2)
        .copied()
        .unwrap_or_default()
        .trim_matches(|c| c == '{' || c == '}'),
    ),
    available_bytes,
    total_bytes,
    steal: None,
    ticks: None,
  }
}

#[cfg(test)]
#[path = "tests/sample_test.rs"]
mod tests;
