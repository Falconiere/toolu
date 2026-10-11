//! One plugin's `hooks/hooks.json`, judged against the native launcher and the
//! Bun launcher.

use std::io::ErrorKind;
use std::path::Path;

use serde_json::{Map, Value};
use toolu_protocol::launcher::{MARKER, MAX_TIMEOUT, Target, hook, hook_name};

use crate::bun_launcher;
use crate::check_hooks::Finding;

/// Findings for `plugins/<plugin>/hooks/hooks.json`; none when it does not exist.
pub(crate) fn check(root: &Path, plugin: &str) -> Vec<Finding> {
  let file = format!("plugins/{plugin}/hooks/hooks.json");
  let finding = |at: String, problem: String, expected: Option<String>| Finding {
    file: file.clone(),
    at,
    problem,
    expected,
  };
  let text = match std::fs::read_to_string(root.join(&file)) {
    Ok(text) => text,
    Err(err) if err.kind() == ErrorKind::NotFound => return Vec::new(),
    Err(err) => return vec![finding(String::new(), format!("cannot read: {err}"), None)],
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
    for (problem, expected) in judge(root, plugin, event, entry) {
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

/// Problems with one hook object. Legacy script commands are left alone.
fn judge(
  root: &Path,
  plugin: &str,
  event: &str,
  entry: &Map<String, Value>,
) -> Vec<(String, Option<String>)> {
  let command = entry
    .get("command")
    .and_then(Value::as_str)
    .unwrap_or_default();
  if command.contains(MARKER) {
    return judge_native(plugin, event, entry, command);
  }
  if bun_launcher::is_native_like(command) {
    return vec![("unsupported native hook command".to_owned(), None)];
  }
  if !bun_launcher::is_launcher(command, entry.contains_key("commandWindows")) {
    return Vec::new();
  }
  judge_bun(root, plugin, event, entry, command)
}

fn judge_native(
  plugin: &str,
  event: &str,
  entry: &Map<String, Value>,
  command: &str,
) -> Vec<(String, Option<String>)> {
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

fn judge_bun(
  root: &Path,
  plugin: &str,
  event: &str,
  entry: &Map<String, Value>,
  command: &str,
) -> Vec<(String, Option<String>)> {
  let windows = entry
    .get("commandWindows")
    .and_then(Value::as_str)
    .unwrap_or_default();
  let Some(name) = bun_launcher::bundle_name(command, windows) else {
    return vec![(
      "launcher hook names no hooks/dist/<entry>.js bundle".to_owned(),
      None,
    )];
  };
  let generated = match bun_launcher::launch(plugin, event, &name) {
    Ok(generated) => generated,
    Err(err) => return vec![(err, None)],
  };
  let mut problems = Vec::new();
  if entry.get("type").and_then(Value::as_str) != Some("command") {
    let got = entry
      .get("type")
      .map_or_else(|| "null".to_owned(), ToString::to_string);
    problems.push((format!("type must be \"command\", got {got}"), None));
  }
  if command != generated.command {
    problems.push((
      "command differs from the generated launcher".to_owned(),
      Some(generated.command),
    ));
  }
  if windows != generated.windows {
    problems.push((
      "commandWindows differs from the generated launcher".to_owned(),
      Some(generated.windows),
    ));
  }
  let bundle = format!("plugins/{plugin}/hooks/dist/{name}.js");
  if !root.join(&bundle).is_file() {
    problems.push((format!("bundle {bundle} is not committed"), None));
  }
  problems
}

#[cfg(test)]
#[path = "tests/hooks_entries_test.rs"]
mod tests;
