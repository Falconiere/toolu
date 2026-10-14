use serde_json::{Value, json};

use super::rows;

fn status(doc: &Value) -> Vec<(String, String)> {
  rows(doc)
    .iter()
    .map(|row| {
      (
        row["id"].as_str().unwrap().to_owned(),
        row["status"].as_str().unwrap().to_owned(),
      )
    })
    .collect()
}

fn doc([while_waiting, rebase, timeout, late, next_gap, ready]: [u64; 6]) -> Value {
  json!({
    "records": 10,
    "wakeups": {"total": 1, "judgment_free": timeout},
    "prompts": {"decided_checks": 1, "while_waiting": while_waiting, "rebase_without_conflict": rebase},
    "github": {"late_reactions": late, "reaction_ms": {"count": 1}, "next_gap_ms": {"max": next_gap}},
    "ready_to_merged_ms": {"clean_excluding_ci": {"p90": ready}},
  })
}

#[test]
fn a_clean_run_passes_every_measured_budget() {
  let all = status(&doc([0, 0, 0, 0, 180_000, 240_000]));
  assert!(
    all[..6].iter().all(|(_, status)| status == "pass"),
    "{all:?}"
  );
  assert_eq!(all[6], ("idle-engine".to_owned(), "unmeasured".to_owned()));
}

#[test]
fn each_planted_regression_fails_its_own_budget() {
  let cases = [
    (doc([1, 0, 0, 0, 180_000, 1]), "prompts-while-waiting"),
    (doc([0, 1, 0, 0, 180_000, 1]), "rebase-without-conflict"),
    (doc([0, 0, 1, 0, 180_000, 1]), "judgment-free-wakeups"),
    (doc([0, 0, 0, 1, 180_000, 1]), "verdict-reaction"),
    (doc([0, 0, 0, 0, 360_000, 1]), "check-gap"),
    (doc([0, 0, 0, 0, 180_000, 240_001]), "ready-to-merged"),
  ];
  for (doc, id) in cases {
    let failed: Vec<String> = status(&doc)
      .into_iter()
      .filter(|(_, status)| status == "fail")
      .map(|(id, _)| id)
      .collect();
    assert_eq!(failed, [id]);
  }
}

#[test]
fn absent_records_leave_a_budget_unmeasured_not_passed() {
  let empty = json!({"records": 0, "wakeups": {"total": 0, "judgment_free": 0},
    "prompts": {"decided_checks": 0, "while_waiting": 3}});
  assert!(
    status(&empty)
      .iter()
      .all(|(_, status)| status == "unmeasured")
  );
}
