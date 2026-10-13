//! The efficiency budgets of #451, judged on a metrics document. A budget
//! whose records are absent is `unmeasured`, never `pass`.

use serde_json::{Value, json};

/// One check interval, the fixed GitHub schedule.
const CHECK_MS: u64 = crate::watch::CHECK_MS;
/// First ready to merged, excluding CI: one interval plus the merge.
const READY_MS: u64 = 240_000;

/// One budget: `(id, budget text, measured value, limit)`; `None` when absent.
struct Budget {
  id: &'static str,
  text: &'static str,
  value: Option<u64>,
  limit: u64,
}

/// The budget rows of `doc`.
pub(crate) fn rows(doc: &Value) -> Vec<Value> {
  let num = |pointer: &str| doc.pointer(pointer).and_then(Value::as_u64);
  let counted = |pointer: &str, gate: &str| num(gate).filter(|n| *n > 0).and(num(pointer));
  let budgets = [
    Budget {
      id: "judgment-free-wakeups",
      text: "0",
      value: counted("/wakeups/judgment_free", "/wakeups/total"),
      limit: 0,
    },
    Budget {
      id: "prompts-while-waiting",
      text: "0",
      value: counted("/prompts/while_waiting", "/prompts/decided_checks"),
      limit: 0,
    },
    Budget {
      id: "rebase-without-conflict",
      text: "0",
      value: counted("/prompts/rebase_without_conflict", "/records"),
      limit: 0,
    },
    Budget {
      id: "verdict-reaction",
      text: "0 reactions that waited for another check",
      value: counted("/github/late_reactions", "/github/reaction_ms/count"),
      limit: 0,
    },
    Budget {
      id: "check-gap",
      text: "next check at most 180 s out",
      value: num("/github/next_gap_ms/max"),
      limit: CHECK_MS,
    },
    Budget {
      id: "ready-to-merged",
      text: "p90 at most 240 s, CI excluded",
      value: num("/ready_to_merged_ms/clean_excluding_ci/p90"),
      limit: READY_MS,
    },
  ];
  let mut rows: Vec<Value> = budgets.iter().map(row).collect();
  rows.push(json!({
    "id": "idle-engine",
    "budget": "at most 10 MB resident, no timer under 30 s",
    "value": null,
    "status": "unmeasured",
    "reason": "asserted by tests/idle_budget.rs; the journal holds only a lifetime peak",
  }));
  rows
}

fn row(budget: &Budget) -> Value {
  let (status, reason) = match budget.value {
    None => ("unmeasured", "no records for this measure"),
    Some(value) if value <= budget.limit => ("pass", ""),
    Some(_) => ("fail", "over budget"),
  };
  json!({
    "id": budget.id,
    "budget": budget.text,
    "value": budget.value,
    "status": status,
    "reason": reason,
  })
}

#[cfg(test)]
#[path = "tests/budgets_test.rs"]
mod tests;
