use serde_json::json;

use super::{MIB, Sample, percentile, summarize, table, violations};

fn row(cpu_us: u64, rss: u64) -> super::Row {
  let sample = Sample {
    max_rss_bytes: rss,
    cpu_us,
    wall_us: cpu_us,
  };
  summarize("toolu/pre-tools", "PreToolUse", &[sample; 3])
}

#[test]
fn percentiles_are_nearest_rank() {
  let values = [5, 1, 4, 2, 3, 10, 9, 8, 7, 6];
  assert_eq!(percentile(&values, 50), 5);
  assert_eq!(percentile(&values, 90), 9);
  assert_eq!(percentile(&[7], 90), 7);
  assert_eq!(percentile(&[], 50), 0);
}

#[test]
fn the_table_shows_mib_and_ms_with_one_decimal() {
  let text = table(&[row(4_250, 6 * MIB)]);
  assert!(
    text.contains("| toolu/pre-tools | PreToolUse | rust | 6.0 / 6.0 | 4.3 / 4.3 | 4.3 / 4.3 |"),
    "{text}"
  );
}

#[test]
fn bench_hooks_lowered_budget_fails() {
  let budgets = json!({"toolu/pre-tools": {"maxRssMiB": 8, "cpuMs": 5}});
  let budgets = budgets.as_object().unwrap();
  assert_eq!(
    violations(&[row(4_000, 6 * MIB)], budgets),
    Vec::<String>::new()
  );
  let lowered = json!({"toolu/pre-tools": {"maxRssMiB": 5, "cpuMs": 3}});
  assert_eq!(
    violations(&[row(4_000, 6 * MIB)], lowered.as_object().unwrap()),
    [
      "toolu/pre-tools: cpu p50 4.0 ms > budget 3 ms",
      "toolu/pre-tools: rss p50 6.0 MiB > budget 5 MiB",
    ]
  );
  let fractional = json!({"toolu/pre-tools": {"maxRssMiB": 5.5, "cpuMs": 3}});
  assert_eq!(
    violations(&[row(1, 1)], fractional.as_object().unwrap()),
    ["toolu/pre-tools: the budget needs whole cpuMs and maxRssMiB"]
  );
  let ungated = json!({});
  assert_eq!(
    violations(&[row(9_999_999, 99 * MIB)], ungated.as_object().unwrap()),
    Vec::<String>::new()
  );
}
