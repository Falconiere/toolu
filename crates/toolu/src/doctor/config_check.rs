//! The config check: the loader's envelope, epic settings and secrets.json.

use std::path::Path;

use serde_json::{Value, json};
use toolu_runtime::config::epic;
use toolu_runtime::config::load::{self, ConfigFiles};
use toolu_runtime::config::secrets::{self, Secrets};
use toolu_runtime::host::roots::Roots;

use super::checks::{Check, Status};

/// Envelope, epic and secrets. `merged` is present only when secrets load.
pub(super) fn check(
  roots: &Roots,
  cwd: &Path,
  secrets: &Result<Secrets, secrets::SecretError>,
) -> Check {
  let loaded = load::load(roots, Some(cwd));
  let warnings = loaded.take_warnings();
  let mut failures = Vec::new();
  if let Some(invalid) = &loaded.invalid {
    failures.push(invalid.clone());
  } else if let Err(error) = epic::parse(&loaded) {
    failures.push(error.to_string());
  }
  if let Err(error) = secrets {
    failures.push(error.to_string());
  }
  let mut details = serde_json::Map::new();
  details.insert("files".to_owned(), existing_files(&loaded.files));
  if let Ok(loaded_secrets) = secrets {
    let merged = secrets::redact_json(&Value::Object(loaded.data), loaded_secrets);
    details.insert("merged".to_owned(), merged);
  }
  let (status, summary) = if !failures.is_empty() {
    (Status::Fail, failures.join("; "))
  } else if !warnings.is_empty() {
    (Status::Warn, warnings.join("; "))
  } else {
    (Status::Ok, "config is valid".to_owned())
  };
  Check::new("config", status, summary, None, Value::Object(details))
}

fn existing_files(files: &ConfigFiles) -> Value {
  let mut paths = Vec::new();
  if load::is_file(&files.user) {
    paths.push(json!(files.user.display().to_string()));
  }
  if let Some(project) = files.project.as_ref().filter(|path| load::is_file(path)) {
    paths.push(json!(project.display().to_string()));
  }
  Value::Array(paths)
}

#[cfg(test)]
#[path = "tests/config_check_test.rs"]
mod tests;
