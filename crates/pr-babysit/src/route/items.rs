//! The fix items a route plan reads, and the tier each gets without Jev.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::error::{Failure, Result};
use crate::pattern::{is, pattern};

pattern!(
  CRITICAL,
  r"\b(security|injection|race|concurrency|deadlock|data[- ]loss)\b"
);
pattern!(
  TRIVIAL,
  r"\b(typos?|nits?|wording|renam(e|es|ed|ing)|spelling|whitespace|comments?|docs?|imports?)\b"
);

const KINDS: [&str; 4] = ["thread", "conversation", "review", "ci"];

/// One review fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
  /// The thread, comment or check id.
  pub(crate) id: String,
  /// What to fix.
  pub(crate) task: String,
  /// `path:line`, when the item has a place.
  pub(crate) path: Option<String>,
  /// The reviewer's severity.
  pub(crate) severity: Option<String>,
  /// thread, conversation, review or ci.
  pub(crate) kind: String,
}

fn invalid(message: &str) -> Failure {
  Failure::new("plan_invalid", message)
}

fn text(item: &Value, key: &str) -> Option<String> {
  item.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn item(value: &Value) -> Result<Item> {
  let id = text(value, "id").filter(|id| value.is_object() && !id.is_empty());
  let id = id.ok_or_else(|| invalid("every item needs an id"))?;
  Ok(Item {
    id,
    task: text(value, "task").unwrap_or_default(),
    path: text(value, "path"),
    severity: text(value, "severity"),
    kind: text(value, "kind").unwrap_or_default(),
  })
}

/// The items of an items document, checked as the TypeScript router checks them.
///
/// # Errors
/// `plan_invalid` naming the first rule an item, the round or `raised` breaks.
pub(crate) fn parse(document: &Value, raised: &[String]) -> Result<Vec<Item>> {
  let list = document.get("items").and_then(Value::as_array);
  let list = list.filter(|list| !list.is_empty());
  let list = list.ok_or_else(|| invalid("items must be a non-empty array"))?;
  let items: Vec<Item> = list.iter().map(item).collect::<Result<_>>()?;
  let ids: BTreeSet<&str> = items.iter().map(|item| item.id.as_str()).collect();
  if ids.len() != items.len() {
    return Err(invalid("item ids must be unique"));
  }
  if items.iter().any(|item| item.task.is_empty()) {
    return Err(invalid("every item needs a task"));
  }
  if items
    .iter()
    .any(|item| !KINDS.contains(&item.kind.as_str()))
  {
    return Err(invalid(
      "item kind must be thread, conversation, review or ci",
    ));
  }
  let round = document.get("round").map_or(Some(1), Value::as_u64);
  if round.is_none_or(|round| round < 1) {
    return Err(invalid("round must be a positive integer"));
  }
  if raised.iter().any(|id| !ids.contains(id.as_str())) {
    return Err(invalid(
      "--raise names an item that is not in the items file",
    ));
  }
  Ok(items)
}

/// The tier index the item's severity and wording suggest.
pub(crate) fn heuristic(item: &Item) -> usize {
  let severity = item.severity.as_deref().unwrap_or_default().to_lowercase();
  let task = item.task.to_lowercase();
  if severity == "critical" || is(&CRITICAL, &task) {
    return 3;
  }
  if severity == "high" {
    return 2;
  }
  if ["low", "nit", ""].contains(&severity.as_str()) && is(&TRIVIAL, &task) {
    return 0;
  }
  1
}

#[cfg(test)]
#[path = "tests/items_test.rs"]
mod tests;
