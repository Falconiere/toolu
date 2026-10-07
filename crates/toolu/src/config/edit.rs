//! `toolu config set`: dotted writes that leave a refused file byte-identical.

use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::atomic::write_atomic;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::config::epic;
use toolu_runtime::config::load::{self, ConfigFiles, LoadedConfig, check_text};
use toolu_runtime::config::secrets::{self, Secrets};
use toolu_runtime::host::roots::Roots;

use super::place;

/// Set `key` to `raw` in the user file, or the project file when `project` is set.
pub(super) fn set(ctx: &Ctx, key: &str, raw: &str, project: bool) -> Outcome {
  if key.split('.').any(str::is_empty) {
    return Outcome::failed(
      Exit::Usage,
      format!("toolu config set: empty path segment in '{key}'"),
    );
  }
  let (roots, cwd) = place(ctx);
  let files = load::config_files(&roots, Some(&cwd));
  let Some(path) = target(&files, project) else {
    return Outcome::failed(
      Exit::Failure,
      "toolu config set: no project config".to_owned(),
    );
  };
  let mut document = match read_document(&path) {
    Ok(document) => document,
    Err(message) => return Outcome::failed(Exit::Failure, message),
  };
  let value = parse_value(raw);
  if let Err(message) = insert(&mut document, key, key, value.clone()) {
    return Outcome::failed(Exit::Failure, message);
  }
  match commit(ctx, &path, &document) {
    Ok(()) => wrote(ctx, &roots, key, &path, &value),
    Err(message) => Outcome::failed(Exit::Failure, message),
  }
}

fn target(files: &ConfigFiles, project: bool) -> Option<std::path::PathBuf> {
  if project {
    files.project.clone()
  } else {
    Some(files.user.clone())
  }
}

fn read_document(path: &Path) -> Result<Map<String, Value>, String> {
  if !load::is_file(path) {
    let mut document = Map::new();
    document.insert("version".to_owned(), json!(1));
    return Ok(document);
  }
  let text = std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
  check_text(&text).map_err(|reason| format!("{}: {reason}", path.display()))
}

fn parse_value(raw: &str) -> Value {
  serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned()))
}

fn insert(
  cursor: &mut Map<String, Value>,
  key: &str,
  rest: &str,
  value: Value,
) -> Result<(), String> {
  let (part, tail) = rest.split_once('.').unwrap_or((rest, ""));
  if tail.is_empty() {
    cursor.insert(part.to_owned(), value);
    return Ok(());
  }
  if !cursor.contains_key(part) {
    cursor.insert(part.to_owned(), Value::Object(Map::new()));
  }
  match cursor.get_mut(part) {
    Some(Value::Object(child)) => insert(child, key, tail, value),
    _ => Err(format!(
      "toolu config set: '{key}' meets a non-object at '{part}'"
    )),
  }
}

fn commit(ctx: &Ctx, path: &Path, document: &Map<String, Value>) -> Result<(), String> {
  let text = serde_json::to_string(document)
    .map_err(|_err| "toolu config set: could not encode the config".to_owned())?;
  let checked = check_text(&text).map_err(|reason| format!("{}: {reason}", path.display()))?;
  let host = ctx.host.unwrap_or(Host::Claude);
  epic::parse(&LoadedConfig::from_data(checked, host)).map_err(|err| err.to_string())?;
  let body = format!("{text}\n");
  if write_atomic(path, &body) {
    Ok(())
  } else {
    Err(format!(
      "toolu config set: could not write {}",
      path.display()
    ))
  }
}

fn wrote(ctx: &Ctx, roots: &Roots, key: &str, path: &Path, value: &Value) -> Outcome {
  let secrets = secrets::load(roots).unwrap_or_else(|_| Secrets::none());
  let value = secrets::redact_json(value, &secrets);
  if ctx.json {
    let document = json!({
      "namespace": "config",
      "key": key,
      "value": value,
      "file": path.display().to_string(),
    });
    return Outcome::data(document.to_string());
  }
  Outcome::data(format!("toolu config set: wrote {key}"))
}

#[cfg(test)]
#[path = "tests/edit_test.rs"]
mod tests;
