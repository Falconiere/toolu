//! Summaries, the Markdown table and the budget verdicts of `cargo xtask bench-hooks`.

use serde::Serialize;
use serde_json::Value;

/// One MiB.
pub(crate) const MIB: u64 = 1024 * 1024;

/// One measured spawn: `cargo xtask measure`'s report, reduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sample {
  pub(crate) max_rss_bytes: u64,
  pub(crate) cpu_us: u64,
  pub(crate) wall_us: u64,
}

/// p50 and p90 of one measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct Spread {
  pub(crate) p50: u64,
  pub(crate) p90: u64,
}

/// One entry's row of the `toolu.hook-resources/v1` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Row {
  pub(crate) entry: String,
  pub(crate) event: String,
  pub(crate) implementation: &'static str,
  pub(crate) max_rss_bytes: Spread,
  pub(crate) cpu_us: Spread,
  pub(crate) wall_us: Spread,
}

/// Nearest-rank percentile `p` of `values` (none: 0).
pub(crate) fn percentile(values: &[u64], p: u64) -> u64 {
  let mut sorted = values.to_vec();
  sorted.sort_unstable();
  let len = u64::try_from(sorted.len()).unwrap_or(u64::MAX);
  let rank = (p * len).div_ceil(100).max(1);
  usize::try_from(rank - 1)
    .ok()
    .and_then(|index| sorted.get(index).copied())
    .unwrap_or(0)
}

fn spread(values: &[u64]) -> Spread {
  Spread {
    p50: percentile(values, 50),
    p90: percentile(values, 90),
  }
}

/// The row of `entry` over `samples`.
pub(crate) fn summarize(entry: &str, event: &str, samples: &[Sample]) -> Row {
  let pick = |of: fn(&Sample) -> u64| samples.iter().map(of).collect::<Vec<_>>();
  Row {
    entry: entry.to_owned(),
    event: event.to_owned(),
    implementation: "rust",
    max_rss_bytes: spread(&pick(|s| s.max_rss_bytes)),
    cpu_us: spread(&pick(|s| s.cpu_us)),
    wall_us: spread(&pick(|s| s.wall_us)),
  }
}

/// Bytes as MiB and microseconds as milliseconds, one decimal.
fn tenths(value: u64, unit: u64) -> String {
  let tenth = (value * 10 + unit / 2) / unit;
  format!("{}.{}", tenth / 10, tenth % 10)
}

/// One Markdown row per entry: RSS in MiB, CPU and wall in ms, p50 / p90.
pub(crate) fn table(rows: &[Row]) -> String {
  let mut lines = vec![
    "| Entry | Event | Impl | RSS p50 / p90 (MiB) | CPU p50 / p90 (ms) | Wall p50 / p90 (ms) |"
      .to_owned(),
    "|---|---|---|---|---|---|".to_owned(),
  ];
  for row in rows {
    lines.push(format!(
      "| {} | {} | {} | {} / {} | {} / {} | {} / {} |",
      row.entry,
      row.event,
      row.implementation,
      tenths(row.max_rss_bytes.p50, MIB),
      tenths(row.max_rss_bytes.p90, MIB),
      tenths(row.cpu_us.p50, 1000),
      tenths(row.cpu_us.p90, 1000),
      tenths(row.wall_us.p50, 1000),
      tenths(row.wall_us.p90, 1000),
    ));
  }
  lines.join("\n") + "\n"
}

/// One line per key of `budgets` that no discovered hook entry (`known`, before any
/// `--only` filter) has: a renamed or removed hook must not leave a budget gating nothing.
pub(crate) fn stale_budgets(
  known: &[String],
  budgets: &serde_json::Map<String, Value>,
) -> Vec<String> {
  let mut stale: Vec<String> = budgets
    .keys()
    .filter(|key| !known.contains(key))
    .map(|key| format!("{key}: a budget for an entry no hooks.json registers"))
    .collect();
  stale.sort();
  stale
}

/// One line per budget a measured entry exceeds. `budgets` is the `entries`
/// object of `benchmarks/hook-budgets.json` (whole MiB and ms); an entry without
/// a budget is not gated.
pub(crate) fn violations(rows: &[Row], budgets: &serde_json::Map<String, Value>) -> Vec<String> {
  let mut found = Vec::new();
  for row in rows {
    let Some(budget) = budgets.get(&row.entry) else {
      continue;
    };
    let limit = |key: &str| budget.get(key).and_then(Value::as_u64);
    let (Some(cpu_ms), Some(rss_mib)) = (limit("cpuMs"), limit("maxRssMiB")) else {
      found.push(format!(
        "{}: the budget needs whole cpuMs and maxRssMiB",
        row.entry
      ));
      continue;
    };
    if row.cpu_us.p50 > cpu_ms * 1000 {
      let got = tenths(row.cpu_us.p50, 1000);
      found.push(format!(
        "{}: cpu p50 {got} ms > budget {cpu_ms} ms",
        row.entry
      ));
    }
    if row.max_rss_bytes.p50 > rss_mib * MIB {
      let got = tenths(row.max_rss_bytes.p50, MIB);
      found.push(format!(
        "{}: rss p50 {got} MiB > budget {rss_mib} MiB",
        row.entry
      ));
    }
  }
  found
}

#[cfg(test)]
#[path = "tests/bench_hooks_report_test.rs"]
mod tests;
