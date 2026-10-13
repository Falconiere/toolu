//! The human form of `toolu epic metrics`: each measure beside the
//! TypeScript baseline committed in `benchmarks/epic-baseline.json`, then the
//! budget table.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde_json::Value;

use crate::metrics::human;

/// The committed baseline, embedded at build time.
pub(crate) const BASELINE: &str = include_str!("../../../../benchmarks/epic-baseline.json");

/// The baseline document, or `null` when it does not parse.
pub(crate) fn baseline() -> Value {
  serde_json::from_str(BASELINE).unwrap_or(Value::Null)
}

/// One `(measure, this run, baseline)` row per compared figure.
pub(crate) fn compared(doc: &Value, base: &Value) -> Vec<[String; 3]> {
  let main = base.get("main").unwrap_or(&Value::Null);
  let mut rows = headline(doc, main);
  let phases: BTreeSet<&String> = [doc, main]
    .iter()
    .filter_map(|value| value.get("phases").and_then(Value::as_object))
    .flat_map(|phases| phases.keys())
    .collect();
  for phase in phases {
    let share = |value: &Value| {
      value
        .pointer(&format!("/phases/{phase}/share"))
        .and_then(Value::as_f64)
        .map_or("—".to_owned(), |share| format!("{:.1}%", share * 100.0))
    };
    rows.push([format!("Phase share: {phase}"), share(doc), share(main)]);
  }
  rows
}

fn ms(value: &Value, pointer: &str) -> Option<u64> {
  value.pointer(pointer).and_then(Value::as_u64)
}

fn pair(value: &Value, prefix: &str) -> String {
  match (
    ms(value, &format!("{prefix}/p50")),
    ms(value, &format!("{prefix}/p90")),
  ) {
    (Some(p50), Some(p90)) => format!("{} / {}", human(p50), human(p90)),
    _ => "—".to_owned(),
  }
}

fn count(value: &Value, pointer: &str) -> String {
  ms(value, pointer).map_or("—".to_owned(), |n| n.to_string())
}

/// The scalar rows: issues, lead time, ready to merged, rebases, relaunches.
fn headline(doc: &Value, main: &Value) -> Vec<[String; 3]> {
  let rounds = match (
    ms(doc, "/rebase_rounds/worker"),
    ms(doc, "/rebase_rounds/engine"),
  ) {
    (Some(worker), Some(engine)) => format!("{worker} by workers, {engine} by the engine"),
    _ => "—".to_owned(),
  };
  vec![
    [
      "Merged issues".to_owned(),
      count(doc, "/lead_time_ms/count"),
      count(main, "/merged_issues"),
    ],
    [
      "Lead time p50 / p90".to_owned(),
      pair(doc, "/lead_time_ms"),
      pair(main, "/lead_time_ms"),
    ],
    [
      "First ready to merged p50 / p90 (baseline: to cleanup)".to_owned(),
      pair(doc, "/ready_to_merged_ms/total"),
      pair(main, "/ready_to_cleanup_ms"),
    ],
    [
      "Rebase rounds".to_owned(),
      rounds,
      format!("{} by workers", count(main, "/rebase_rounds")),
    ],
    [
      "Relaunches (baseline: relaunched issues)".to_owned(),
      count(doc, "/launches/relaunches"),
      count(main, "/relaunched_issues"),
    ],
  ]
}

/// The text of `toolu epic metrics`.
pub(crate) fn render(doc: &Value, budgets: &[Value]) -> String {
  let scope = doc
    .get("epic")
    .and_then(Value::as_str)
    .unwrap_or("every epic");
  let mut out = format!(
    "Epic metrics for {scope}: {} records, {} issues\n\n",
    doc.get("records").unwrap_or(&Value::Null),
    doc.get("issues").unwrap_or(&Value::Null)
  );
  let _written = writeln!(out, "{:<58} {:<26} Baseline", "Measure", "This run");
  for [measure, run, base] in compared(doc, &baseline()) {
    let _written = writeln!(out, "{measure:<58} {run:<26} {base}");
  }
  out.push_str("\nBudgets\n");
  for row in budgets {
    let text = |key: &str| row.get(key).and_then(Value::as_str).unwrap_or("");
    let value = row.get("value").filter(|value| !value.is_null());
    let value = value.map_or("—".to_owned(), Value::to_string);
    let _written = writeln!(
      out,
      "{:<26} {:<10} {:<8} {}",
      text("id"),
      text("status"),
      value,
      text("budget")
    );
  }
  out
}

#[cfg(test)]
#[path = "tests/text_test.rs"]
mod tests;
