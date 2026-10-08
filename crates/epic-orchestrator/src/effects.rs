//! Script effects. `TOOLU_EPIC_SCRIPTS/<name>` prints one JSON object.
//! Without that directory the action is `deferred` and nothing is executed.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use toolu_runtime::process::{Spec, run};

use crate::model::Action;

/// Run `action`'s script, or `deferred` when `scripts` is unset.
pub(crate) fn invoke(scripts: Option<&Path>, action: Action, stdin_json: &str) -> String {
  let Some(dir) = scripts else {
    return "deferred".to_owned();
  };
  let path = dir.join(action.name());
  if !path.is_file() {
    return "failed".to_owned();
  }
  let script = path.display().to_string();
  let mut spec = Spec::new([String::from("/bin/sh"), script]);
  spec.stdin = stdin_json.as_bytes().to_vec();
  spec.timeout = Duration::from_secs(30);
  match run(&spec) {
    Ok(output) if output.exit_code == 0 => parse_outcome(&output.stdout, action),
    _ => "failed".to_owned(),
  }
}

fn parse_outcome(stdout: &str, action: Action) -> String {
  let Ok(value) = serde_json::from_str::<Value>(stdout.trim()) else {
    return "failed".to_owned();
  };
  let key = if action == Action::Reconcile {
    "state"
  } else {
    "outcome"
  };
  value
    .get(key)
    .and_then(Value::as_str)
    .unwrap_or("failed")
    .to_owned()
}

#[cfg(test)]
#[path = "tests/effects_test.rs"]
mod tests;
