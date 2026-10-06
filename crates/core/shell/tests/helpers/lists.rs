//! String lists inside a fixture case, for the replays of `fixture_cases.rs`.

use serde_json::Value;

use crate::cases::Res;

/// The list of strings `key` of `case`.
pub(crate) fn strings(case: &Value, key: &str) -> Res<Vec<String>> {
  let items = case.get(key).and_then(Value::as_array);
  let items = items.ok_or_else(|| format!("no {key} list in {case}"))?;
  items
    .iter()
    .map(|item| {
      let text = item.as_str();
      text
        .map(str::to_owned)
        .ok_or_else(|| format!("{key} holds {item} in {case}"))
    })
    .collect()
}
