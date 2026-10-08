//! Read-only join of a delegation to the current plan step.

use std::path::Path;

use super::value_text;
use toolu_engine::ledger::io::read_ledger;
use toolu_engine::ledger::jq::get;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::git::{branch_slug, current_branch};

fn steps(ledger: &Ordered) -> Vec<&Ordered> {
  let Some(value) = get(ledger, "steps").ok() else {
    return Vec::new();
  };
  match value {
    Ordered::Array(items) => items.iter().collect(),
    Ordered::Object(entries) => entries.iter().map(|(_, value)| value).collect(),
    Ordered::Null | Ordered::Bool(_) | Ordered::Number(_) | Ordered::String(_) => Vec::new(),
  }
}

fn step_join(ledger: &Ordered) -> (String, String) {
  let steps = steps(ledger);
  let running = steps.iter().find(|step| {
    get(step, "status").is_ok_and(|status| *status == Ordered::String("running".to_owned()))
  });
  let next = get(ledger, "next").map(value_text).unwrap_or_default();
  let id = running
    .and_then(|step| get(step, "id").ok())
    .map(value_text)
    .filter(|id| !id.is_empty())
    .unwrap_or(next);
  if id.is_empty() {
    return (id, String::new());
  }
  let tier = steps
    .iter()
    .find(|step| get(step, "id").is_ok_and(|value| value_text(value) == id))
    .and_then(|step| get(step, "model").ok())
    .map(value_text)
    .unwrap_or_default();
  (id, tier)
}

/// The checked-out branch's running step, else its next step, and declared model.
pub(super) fn join(root: &Path, roots: &Roots) -> (String, String) {
  let branch = current_branch(roots.env(), root);
  if branch.is_empty() || branch == "HEAD" {
    return (String::new(), String::new());
  }
  let dir = roots
    .env()
    .get("LEDGER_DIR")
    .map(std::path::PathBuf::from)
    .or_else(|| {
      roots
        .project_state_dir("plan-ledger", None, Some(root))
        .ok()
        .flatten()
    });
  let Some(dir) = dir else {
    return (String::new(), String::new());
  };
  let file = dir.join(format!("{}.json", branch_slug(&branch)));
  read_ledger(&file).map_or_else(
    || (String::new(), String::new()),
    |ledger| step_join(&ledger.value),
  )
}

#[cfg(test)]
#[path = "tests/agent_tier_ledger_test.rs"]
mod tests;
