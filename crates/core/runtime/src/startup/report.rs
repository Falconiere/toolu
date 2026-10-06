//! The startup report (`report.ts`). A host that runs `SessionStart` entries
//! itself, like the `OpenCode` bootstrap, cannot tell a failed registry write or
//! a helper with a missing source from a success. When `TOOLU_STARTUP_REPORT`
//! names a file, each contribution appends one JSON line there; unset, nothing
//! is written.

use std::io::Write as _;

use serde::Serialize;

use crate::env::Env;
use crate::registry::RegistryEvent;

/// The variable that names the report file.
pub const STARTUP_REPORT_ENV: &str = "TOOLU_STARTUP_REPORT";

/// How a registry write ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RegistryStatus {
  /// The file was written.
  Written,
  /// The file already held these bytes.
  Unchanged,
  /// The write failed.
  Failed,
}

/// How a stable-path publish ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HelperStatus {
  /// The link points at the source.
  Published,
  /// A user's file or directory holds the path and was left alone.
  KeptUserFile,
  /// The link could not be made.
  LinkFailed,
  /// The directory could not be created.
  Unwritable,
  /// There was nothing to publish.
  SourceMissing,
}

/// One line of the report, in TypeScript's key order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum StartupRecord {
  /// One registry module of a plugin's `register` entry.
  Registry {
    /// The owning plugin.
    spec: String,
    /// The module name.
    name: String,
    /// Its event.
    event: RegistryEvent,
    /// The file published.
    source: String,
    /// Where it was published.
    target: String,
    /// How it ended.
    status: RegistryStatus,
    /// Why it failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
  },
  /// One stable-path helper.
  Helper {
    /// The plugin.
    plugin: String,
    /// The file published.
    source: String,
    /// Where; absent only when there was no source.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    /// How it ended.
    status: HelperStatus,
  },
  /// A failure outside any one contribution.
  Error {
    /// Where it happened.
    origin: String,
    /// What happened.
    message: String,
  },
}

/// Appends `record` to the report file when `env` names one. A record that
/// cannot be written must never read as success.
///
/// # Errors
/// `toolu-startup: cannot write startup report <path>: <reason>`, for the
/// caller to print before it exits 1 once its work is done.
pub fn report(env: &Env, record: &StartupRecord) -> Result<(), String> {
  let Some(path) = env.get(STARTUP_REPORT_ENV) else {
    return Ok(());
  };
  let fail =
    |reason: String| format!("toolu-startup: cannot write startup report {path}: {reason}");
  let line = serde_json::to_string(record).map_err(|err| fail(err.to_string()))? + "\n";
  let mut file = std::fs::OpenOptions::new()
    .append(true)
    .create(true)
    .open(path)
    .map_err(|err| fail(err.to_string()))?;
  file
    .write_all(line.as_bytes())
    .map_err(|err| fail(err.to_string()))
}

#[cfg(test)]
#[path = "tests/report_test.rs"]
mod tests;
