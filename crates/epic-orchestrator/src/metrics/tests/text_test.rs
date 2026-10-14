use serde_json::json;

use super::{baseline, compared, render};

#[test]
fn each_measure_sits_beside_the_baseline() {
  let doc = json!({
    "epic": "e1", "records": 3, "issues": 1,
    "lead_time_ms": {"count": 1, "p50": 600_000, "p90": 3_600_000},
    "ready_to_merged_ms": {"total": {"p50": 30_000, "p90": 90_000}},
    "rebase_rounds": {"engine": 2, "worker": 0},
    "launches": {"relaunches": 0},
    "phases": {"execution": {"ms": 1, "share": 0.5}},
  });
  let rows = compared(&doc, &baseline());
  assert_eq!(
    rows[0],
    ["Merged issues".to_owned(), "1".to_owned(), "83".to_owned()]
  );
  assert_eq!(rows[1][1], "10 min / 1 h 0 min");
  assert_eq!(rows[1][2], "3 h 39 min / 15 h 24 min");
  assert_eq!(rows[3][1], "0 by workers, 2 by the engine");
  assert_eq!(rows[3][2], "119 by workers");
  let execution = rows
    .iter()
    .find(|row| row[0] == "Phase share: execution")
    .unwrap();
  assert_eq!(execution[1], "50.0%");
  assert_eq!(execution[2], "38.7%");
  let text = render(
    &doc,
    &[json!({"id": "check-gap", "status": "pass", "value": 180_000, "budget": "b"})],
  );
  assert!(
    text.starts_with("Epic metrics for e1: 3 records, 1 issues"),
    "{text}"
  );
  assert!(text.contains("check-gap"), "{text}");
}

#[test]
fn absent_figures_print_a_dash() {
  let rows = compared(&json!({}), &json!({}));
  assert_eq!(
    rows[1],
    [
      "Lead time p50 / p90".to_owned(),
      "—".to_owned(),
      "—".to_owned()
    ]
  );
}
