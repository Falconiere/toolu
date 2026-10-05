//! Each plugin manifest declares the binary's `hookProtocol` (#411).

use std::path::Path;

use serde_json::Value;
use toolu_protocol::HOOK_PROTOCOL;

use crate::check_hooks::Finding;

/// Findings for the Claude and Codex manifests of `plugin` that exist.
pub(crate) fn check(root: &Path, plugin: &str) -> Vec<Finding> {
  [".claude-plugin", ".codex-plugin"]
    .iter()
    .filter_map(|dir| {
      let file = format!("plugins/{plugin}/{dir}/plugin.json");
      let text = std::fs::read_to_string(root.join(&file)).ok()?;
      let problem = problem(&text)?;
      Some(Finding {
        file,
        at: String::new(),
        problem,
        expected: None,
      })
    })
    .collect()
}

/// What is wrong with one manifest's `hookProtocol`, or `None`.
fn problem(text: &str) -> Option<String> {
  let json = match serde_json::from_str::<Value>(text) {
    Ok(json) => json,
    Err(err) => return Some(format!("invalid JSON: {err}")),
  };
  match json.get("hookProtocol").and_then(Value::as_u64) {
    Some(protocol) if protocol == u64::from(HOOK_PROTOCOL) => None,
    Some(protocol) => Some(format!(
      "hookProtocol is {protocol}, but toolu speaks {HOOK_PROTOCOL}"
    )),
    None => Some(format!(
      "no integer hookProtocol; add \"hookProtocol\": {HOOK_PROTOCOL}"
    )),
  }
}

#[cfg(test)]
#[path = "tests/hooks_manifests_test.rs"]
mod tests;
