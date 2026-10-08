//! The status document `toolu epic status --json` prints.

use serde_json::{Value, json};

use crate::model::{Issue, World};

/// One status document. `epic` keeps issues for that epic key when set.
pub(crate) fn document(world: &World, running: bool, epic: Option<&str>) -> Value {
  let issues: Vec<&Issue> = world
    .issues
    .values()
    .filter(|issue| wanted(epic, issue))
    .collect();
  json!({
    "engine": if running { "running" } else { "down" },
    "paused": world.paused_all || !world.paused.is_empty(),
    "epics": epic_keys(&issues),
    "issues": issues.iter().map(|issue| json!({
      "key": issue.key,
      "epic": issue.epic,
      "phase": issue.phase,
      "stage": issue.stage,
    })).collect::<Vec<_>>(),
    "attention": world.attention.iter().filter(|item| !item.delivered).map(|item| json!({
      "kind": item.kind,
      "key": item.key,
      "epic": item.epic,
      "note": item.note,
      "seq": item.seq,
    })).collect::<Vec<_>>(),
  })
}

fn wanted(epic: Option<&str>, issue: &Issue) -> bool {
  match epic {
    None => true,
    Some(epic) => issue.epic == epic || issue.epic.ends_with(&format!("-{epic}")),
  }
}

fn epic_keys(issues: &[&Issue]) -> Vec<Value> {
  let mut keys = Vec::new();
  for issue in issues {
    let key = json!({"key": issue.epic});
    if !keys.contains(&key) {
      keys.push(key);
    }
  }
  keys
}

#[cfg(test)]
#[path = "tests/status_test.rs"]
mod tests;
