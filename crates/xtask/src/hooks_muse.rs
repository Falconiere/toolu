//! One plugin's Muse hook wiring (#546): every `capabilities.hooks` entry of
//! `.muse-plugin/plugin.json` must equal `cargo xtask print-muse-hook`'s output,
//! every resolver it runs must be the generated file byte for byte, and no file
//! in `hooks/muse/` may be anything but a referenced resolver.

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::Path;

use serde_json::{Map, Value};
use toolu_protocol::launcher::muse::{
  DEFAULT_TIMEOUT_MS, MAX_TIMEOUT_MS, MuseHook, RESOLVER_DIR, hook,
};
use toolu_protocol::launcher::{Target, is_name};

use crate::check_hooks::Finding;

/// One plugin's manifest, as the entry judge needs it.
struct Plugin<'a> {
  name: &'a str,
  manifest: String,
}

/// Findings for `plugins/<plugin>`'s Muse manifest and resolvers.
pub(crate) fn check(root: &Path, plugin: &str) -> Vec<Finding> {
  let manifest = format!("plugins/{plugin}/.muse-plugin/plugin.json");
  let at_file = |problem: String| Finding {
    file: manifest.clone(),
    at: String::new(),
    problem,
    expected: None,
  };
  let text = match std::fs::read_to_string(root.join(&manifest)) {
    Ok(text) => Some(text),
    Err(err) if err.kind() == ErrorKind::NotFound => None,
    Err(err) => return vec![at_file(format!("cannot read: {err}"))],
  };
  let Some(text) = text else {
    return orphans(root, plugin, &BTreeSet::new());
  };
  let json = match serde_json::from_str::<Value>(&text) {
    Ok(json) => json,
    Err(err) => return vec![at_file(format!("invalid JSON: {err}"))],
  };
  let mut found = Vec::new();
  if json.get("hookProtocol").is_some() {
    found.push(at_file(
      "Muse manifests carry no hookProtocol; skew is checked from .claude-plugin/plugin.json"
        .to_owned(),
    ));
  }
  let hooks = json.pointer("/capabilities/hooks").unwrap_or(&Value::Null);
  let entries = match hooks {
    Value::Null => &[][..],
    Value::Array(entries) => entries.as_slice(),
    Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_) => {
      found.push(at_file("capabilities.hooks is not an array".to_owned()));
      return found;
    }
  };
  let this = Plugin {
    name: plugin,
    manifest,
  };
  let (entry_found, generated, ids) = judge_entries(&this, entries);
  found.extend(entry_found);
  for expected in &generated {
    found.extend(resolver(root, plugin, expected));
  }
  found.extend(orphans(root, plugin, &ids));
  found
}

/// Entry findings, the hooks the valid entries generate, and every valid id.
fn judge_entries(
  plugin: &Plugin<'_>,
  entries: &[Value],
) -> (Vec<Finding>, Vec<MuseHook>, BTreeSet<String>) {
  let mut found = Vec::new();
  let mut generated = Vec::new();
  let mut ids = BTreeSet::new();
  for (i, entry) in entries.iter().enumerate() {
    let finding = |problem: String, expected: Option<String>| Finding {
      file: plugin.manifest.clone(),
      at: format!("capabilities.hooks[{i}]"),
      problem,
      expected,
    };
    let Some(entry) = entry.as_object() else {
      found.push(finding("entry is not an object".to_owned(), None));
      continue;
    };
    let id = entry
      .get("id")
      .and_then(Value::as_str)
      .filter(|id| is_name(id));
    let Some(id) = id else {
      let problem = "entry needs a string id matching ^[a-z0-9]+(-[a-z0-9]+)*$".to_owned();
      found.push(finding(problem, None));
      continue;
    };
    if !ids.insert(id.to_owned()) {
      found.push(finding(format!("duplicate hook id {id}"), None));
      continue;
    }
    match judge(plugin.name, id, entry) {
      Ok((expected, problem)) => {
        if let Some(problem) = problem {
          let text = serde_json::to_string(&expected.entry).ok();
          found.push(finding(problem, text));
        }
        generated.push(expected);
      }
      Err(problem) => found.push(finding(problem, None)),
    }
  }
  (found, generated, ids)
}

/// The hook generated for one entry and the entry's problem, if any; `Err` when
/// nothing can be generated for it.
fn judge(
  plugin: &str,
  id: &str,
  entry: &Map<String, Value>,
) -> Result<(MuseHook, Option<String>), String> {
  let event = entry
    .get("event")
    .and_then(Value::as_str)
    .unwrap_or_default();
  let timeout = entry.get("timeoutMs").and_then(Value::as_u64);
  let timeout = timeout.and_then(|value| u32::try_from(value).ok());
  let timeout = timeout.filter(|value| (1..=MAX_TIMEOUT_MS).contains(value));
  let target = Target {
    plugin,
    event,
    name: id,
  };
  let expected = hook(&target, timeout.unwrap_or(DEFAULT_TIMEOUT_MS))?;
  let fix = fix_command(&target, timeout.unwrap_or(DEFAULT_TIMEOUT_MS));
  let same = serde_json::to_value(&expected.entry).ok() == Some(Value::Object(entry.clone()));
  let problem = if timeout.is_none() {
    Some(format!(
      "timeoutMs must be an integer from 1 to {MAX_TIMEOUT_MS}; run {fix}"
    ))
  } else if same {
    None
  } else {
    Some(format!("entry differs from the generator; run {fix}"))
  };
  Ok((expected, problem))
}

/// The `print-muse-hook` command that prints the expected entry.
fn fix_command(target: &Target<'_>, timeout_ms: u32) -> String {
  let fix = format!(
    "cargo xtask print-muse-hook {} {} {}",
    target.plugin, target.event, target.name
  );
  if timeout_ms == DEFAULT_TIMEOUT_MS {
    fix
  } else {
    format!("{fix} --timeout-ms {timeout_ms}")
  }
}

/// A finding when the resolver `expected` runs is missing or not the generated text.
fn resolver(root: &Path, plugin: &str, expected: &MuseHook) -> Option<Finding> {
  let file = format!("plugins/{plugin}/{}", expected.resolver_path);
  let fix = format!(
    "run cargo xtask print-muse-hook {plugin} {} {} --resolver > {file}",
    expected.entry.event, expected.entry.id
  );
  let problem = match std::fs::read_to_string(root.join(&file)) {
    Ok(text) if text == expected.resolver => return None,
    Ok(_) => format!("resolver differs from the generator; {fix}"),
    Err(err) if err.kind() == ErrorKind::NotFound => format!("resolver is missing; {fix}"),
    Err(err) => format!("cannot read: {err}"),
  };
  Some(Finding {
    file,
    at: String::new(),
    problem,
    expected: None,
  })
}

/// A finding for every file in `hooks/muse/` that no entry id names.
fn orphans(root: &Path, plugin: &str, ids: &BTreeSet<String>) -> Vec<Finding> {
  let dir = format!("plugins/{plugin}/{RESOLVER_DIR}");
  let finding = |file: String, problem: String| Finding {
    file,
    at: String::new(),
    problem,
    expected: None,
  };
  let listing = match std::fs::read_dir(root.join(&dir)) {
    Ok(listing) => listing,
    Err(err) if err.kind() == ErrorKind::NotFound => return Vec::new(),
    Err(err) => return vec![finding(dir, format!("cannot list: {err}"))],
  };
  let mut names = Vec::new();
  for entry in listing {
    match entry {
      Ok(entry) => names.push(entry.file_name().to_string_lossy().into_owned()),
      Err(err) => return vec![finding(dir, format!("cannot list: {err}"))],
    }
  }
  names.retain(|name| !ids.contains(name));
  names.sort();
  names
    .into_iter()
    .map(|name| {
      let problem = "not referenced by .muse-plugin/plugin.json; remove it or add its entry";
      finding(format!("{dir}/{name}"), problem.to_owned())
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/hooks_muse_test.rs"]
mod tests;
