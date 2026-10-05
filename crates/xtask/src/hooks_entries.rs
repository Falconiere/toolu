//! The native entries of one plugin's `hooks/hooks.json`, judged against the
//! launcher generator.

use std::path::Path;

use serde_json::{Map, Value};
use toolu_protocol::launcher::{MARKER, MAX_TIMEOUT, Target, hook, hook_name};

use crate::check_hooks::Finding;

/// Findings for `plugins/<plugin>/hooks/hooks.json`; none when it is absent.
pub(crate) fn check(root: &Path, plugin: &str) -> Vec<Finding> {
  let file = format!("plugins/{plugin}/hooks/hooks.json");
  let Ok(text) = std::fs::read_to_string(root.join(&file)) else {
    return Vec::new();
  };
  let finding = |at: String, problem: String, expected: Option<String>| Finding {
    file: file.clone(),
    at,
    problem,
    expected,
  };
  let events = match serde_json::from_str::<Value>(&text) {
    Ok(json) => json.get("hooks").and_then(Value::as_object).cloned(),
    Err(err) => return vec![finding(String::new(), format!("invalid JSON: {err}"), None)],
  };
  let Some(events) = events else {
    return vec![finding(
      String::new(),
      "no \"hooks\" object".to_owned(),
      None,
    )];
  };
  let mut found = Vec::new();
  for (event, at, entry) in entries(&events) {
    for (problem, expected) in judge(plugin, event, entry) {
      found.push(finding(at.clone(), problem, expected));
    }
  }
  found
}

/// Every hook object with its event and `<Event>[i].hooks[j]` position.
fn entries(events: &Map<String, Value>) -> Vec<(&str, String, &Map<String, Value>)> {
  events
    .iter()
    .flat_map(|(event, groups)| {
      let groups = groups.as_array().map(Vec::as_slice).unwrap_or_default();
      groups.iter().enumerate().flat_map(move |(i, group)| {
        let hooks = group.get("hooks").and_then(Value::as_array);
        let hooks = hooks.map(Vec::as_slice).unwrap_or_default();
        hooks.iter().enumerate().filter_map(move |(j, entry)| {
          let at = format!("{event}[{i}].hooks[{j}]");
          entry.as_object().map(|entry| (event.as_str(), at, entry))
        })
      })
    })
    .collect()
}

/// Problems with one hook object; none for a hook that is not native.
fn judge(plugin: &str, event: &str, entry: &Map<String, Value>) -> Vec<(String, Option<String>)> {
  let command = entry
    .get("command")
    .and_then(Value::as_str)
    .unwrap_or_default();
  if !command.contains(MARKER) {
    return Vec::new();
  }
  let Some(name) = hook_name(command) else {
    return vec![("native entry names no `hook <name>`".to_owned(), None)];
  };
  let timeout = entry.get("timeout").and_then(Value::as_u64);
  let timeout = timeout.and_then(|value| u32::try_from(value).ok());
  let mut problems = Vec::new();
  let Some(timeout) = timeout.filter(|value| (1..=MAX_TIMEOUT).contains(value)) else {
    problems.push((
      format!("timeout must be an integer from 1 to {MAX_TIMEOUT}"),
      None,
    ));
    return problems;
  };
  let expected = match hook(
    &Target {
      plugin,
      event,
      name,
    },
    timeout,
  ) {
    Ok(expected) => expected,
    Err(err) => return vec![(err, None)],
  };
  if entry.get("type").and_then(Value::as_str) != Some("command") {
    problems.push(("type must be \"command\"".to_owned(), None));
  }
  if command != expected.command {
    let problem = "command differs from the generated launcher".to_owned();
    problems.push((problem, Some(expected.command)));
  }
  let windows = entry.get("commandWindows").and_then(Value::as_str);
  if windows != Some(expected.command_windows.as_str()) {
    let problem = "commandWindows differs from the generated launcher".to_owned();
    problems.push((problem, Some(expected.command_windows)));
  }
  problems
}

#[cfg(test)]
#[path = "tests/hooks_entries_test.rs"]
mod tests;
