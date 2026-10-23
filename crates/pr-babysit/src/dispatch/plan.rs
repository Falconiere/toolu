//! A route plan and its items, checked before any worktree or fixer record.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::error::{Failure, Result};
use crate::fixer::host::{Fixer, NAMES};

fn invalid(message: &str) -> Failure {
  Failure::new("plan_invalid", message)
}

/// A JSON object from `path`.
///
/// # Errors
/// `code` with `message` when the file is missing or not a JSON object.
pub(crate) fn read_object(path: &Path, code: &'static str, message: String) -> Result<Value> {
  let text = std::fs::read_to_string(path).unwrap_or_default();
  match serde_json::from_str::<Value>(&text) {
    Ok(value) if value.is_object() => Ok(value),
    Ok(_) | Err(_) => Err(Failure::new(code, message)),
  }
}

fn well_formed(group: &Value) -> bool {
  let items = group.get("items").and_then(Value::as_array);
  group.get("host").is_some_and(|host| !host.is_null())
    && items.is_some_and(|items| items.iter().all(Value::is_string))
}

fn ids(group: &Value) -> impl Iterator<Item = &str> {
  group
    .get("items")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
}

fn hosts(groups: &[Value]) -> Result<()> {
  for group in groups {
    let seq = group.get("seq").and_then(Value::as_u64).unwrap_or_default();
    let host = group
      .get("host")
      .and_then(Value::as_str)
      .unwrap_or_default();
    let fixer = Fixer::ALL.into_iter().find(|fixer| fixer.name() == host);
    let Some(fixer) = fixer else {
      let named = group.get("host").map_or_else(String::new, |host| {
        host
          .as_str()
          .map_or_else(|| host.to_string(), str::to_owned)
      });
      return Err(Failure::new(
        "config_invalid",
        format!("plan group {seq} names host '{named}'; use one of {NAMES}"),
      ));
    };
    let text = |key: &str| group.get(key).and_then(Value::as_str);
    fixer.args("pb-000000-r1g1", text("model"), text("effort"), true)?;
  }
  Ok(())
}

/// The plan's groups, well formed and numbered 1, 2, ….
fn shaped(plan: &Value) -> Result<&[Value]> {
  if plan.get("dispatch").and_then(Value::as_str) != Some("herdr") {
    return Err(invalid(
      "the plan dispatches inline; run this round in-session",
    ));
  }
  let groups = plan
    .get("groups")
    .and_then(Value::as_array)
    .filter(|groups| !groups.is_empty());
  let groups = groups.ok_or_else(|| invalid("the plan has no groups"))?;
  if !groups.iter().all(well_formed) {
    return Err(invalid("every plan group needs a host and string item ids"));
  }
  let seqs: Vec<Option<u64>> = groups
    .iter()
    .map(|group| group.get("seq").and_then(Value::as_u64))
    .collect();
  if seqs.iter().any(|seq| seq.is_none_or(|seq| seq < 1)) {
    return Err(invalid("every plan group needs a positive integer seq"));
  }
  if seqs
    .iter()
    .enumerate()
    .any(|(index, seq)| *seq != u64::try_from(index + 1).ok())
  {
    return Err(invalid("plan groups need contiguous 1-based seq values"));
  }
  Ok(groups)
}

/// Every item the groups name is in `items` and none is injection-flagged.
fn known_items(groups: &[Value], items: &Value, state: &Value) -> Result<()> {
  if crate::json::round(items).is_none() {
    return Err(invalid("round must be a positive integer"));
  }
  let known: BTreeSet<&str> = items
    .get("items")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(|item| item.get("id")?.as_str())
    .collect();
  if groups.iter().flat_map(ids).any(|id| !known.contains(id)) {
    return Err(invalid(
      "the plan names an item that is not in the items file",
    ));
  }
  let flagged = state.pointer("/actions/flagged");
  let is_flagged = |id: &str| {
    flagged
      .and_then(|flagged| flagged.get(id))
      .is_some_and(|entry| !entry.is_null() && entry != &Value::Bool(false))
  };
  if groups.iter().flat_map(ids).any(is_flagged) {
    return Err(invalid("the plan includes an injection-flagged thread"));
  }
  Ok(())
}

/// Checks `plan` against `items` and the slot `state`, as the TypeScript dispatcher does.
///
/// # Errors
/// `plan_invalid` or `config_invalid` naming the first rule broken.
pub(crate) fn validate(plan: &Value, items: &Value, state: &Value) -> Result<()> {
  let groups = shaped(plan)?;
  known_items(groups, items, state)?;
  hosts(groups)
}

#[cfg(test)]
#[path = "tests/plan_test.rs"]
mod tests;
