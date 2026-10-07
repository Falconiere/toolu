//! `toolu config get` and `toolu config validate`.

use std::path::Path;

use serde_json::{Value, json};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::config::epic;
use toolu_runtime::config::load::{self, ConfigFiles, LoadedConfig, check_text};
use toolu_runtime::config::secrets;

use super::place;

/// Print one key, or the redacted merged document when `key` is absent.
pub(super) fn get(ctx: &Ctx, key: Option<&str>) -> Outcome {
  if let Some(key) = key.filter(|key| key.split('.').any(str::is_empty)) {
    return Outcome::failed(
      Exit::Usage,
      format!("toolu config get: empty path segment in '{key}'"),
    );
  }
  let (roots, cwd) = place(ctx);
  let files = load::config_files(&roots, Some(&cwd));
  let secrets = match secrets::load(&roots) {
    Ok(secrets) => secrets,
    Err(err) => return Outcome::failed(Exit::Failure, err.to_string()),
  };
  let loaded = load::load(&roots, Some(&cwd));
  let document = secrets::redact_json(&Value::Object(loaded.data), &secrets);
  match selected(&document, key) {
    Ok(value) => present(ctx, key, &files, &value),
    Err(message) => Outcome::failed(Exit::Failure, message),
  }
}

/// Exit 1 when an envelope, a malformed file or an epic setting is unusable.
pub(super) fn validate(ctx: &Ctx) -> Outcome {
  let (roots, cwd) = place(ctx);
  let files = load::config_files(&roots, Some(&cwd));
  let loaded = load::load(&roots, Some(&cwd));
  let warnings = loaded.take_warnings();
  if let Some(message) = problem(&files, &loaded) {
    return finish(ctx, &files, false, &warnings, Some(message));
  }
  match epic::parse(&loaded) {
    Ok(_) => finish(ctx, &files, true, &warnings, None),
    Err(err) => finish(ctx, &files, false, &warnings, Some(err.to_string())),
  }
}

/// A JSON string is printed raw; every other value is pretty JSON.
pub(super) fn render_value(value: &Value) -> String {
  match value {
    Value::String(text) => text.clone(),
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
      serde_json::to_string_pretty(value).unwrap_or_else(|_err| "null".to_owned())
    }
  }
}

fn selected(document: &Value, key: Option<&str>) -> Result<Value, String> {
  let Some(key) = key else {
    return Ok(document.clone());
  };
  lookup(document, key)
    .cloned()
    .ok_or_else(|| format!("toolu config get: no such key '{key}'"))
}

fn lookup<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
  let mut cursor = value;
  for part in key.split('.') {
    cursor = cursor.get(part)?;
  }
  Some(cursor)
}

fn present(ctx: &Ctx, key: Option<&str>, files: &ConfigFiles, value: &Value) -> Outcome {
  if ctx.json {
    let document = json!({
      "namespace": "config",
      "key": key,
      "value": value,
      "files": files_value(files),
    });
    return Outcome::data(document.to_string());
  }
  Outcome::data(render_value(value))
}

fn problem(files: &ConfigFiles, loaded: &LoadedConfig) -> Option<String> {
  file_error(&files.user)
    .or_else(|| files.project.as_deref().and_then(file_error))
    .or_else(|| loaded.invalid.clone())
}

fn file_error(path: &Path) -> Option<String> {
  if !load::is_file(path) {
    return None;
  }
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(err) => return Some(format!("{}: {err}", path.display())),
  };
  check_text(&text)
    .err()
    .map(|reason| format!("{}: {reason}", path.display()))
}

fn finish(
  ctx: &Ctx,
  files: &ConfigFiles,
  valid: bool,
  warnings: &[String],
  message: Option<String>,
) -> Outcome {
  if ctx.json {
    let document = json!({
      "namespace": "config",
      "valid": valid,
      "files": files_value(files),
      "warnings": warnings,
    });
    return Outcome {
      exit: if valid { Exit::Success } else { Exit::Failure },
      stdout: Some(document.to_string()),
      stderr: message,
    };
  }
  if let Some(message) = message {
    return Outcome::failed(Exit::Failure, message);
  }
  let stderr = (!warnings.is_empty()).then(|| warnings.join("\n"));
  Outcome {
    exit: Exit::Success,
    stdout: Some("toolu config validate: valid".to_owned()),
    stderr,
  }
}

fn files_value(files: &ConfigFiles) -> Value {
  json!({
    "user": files.user.display().to_string(),
    "project": files.project.as_ref().map(|path| path.display().to_string()),
  })
}

#[cfg(test)]
#[path = "tests/show_test.rs"]
mod tests;
