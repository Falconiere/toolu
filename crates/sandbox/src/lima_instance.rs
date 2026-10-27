//! What `limactl` says about itself and a VM: its version, and from `limactl list
//! --format json` (one document per instance and line) the VM's status, size and
//! mounts.

use std::path::PathBuf;

use serde_json::Value;

/// A VM's state as Lima reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
  /// Booted and reachable.
  Running,
  /// Shut down; `start` boots it.
  Stopped,
  /// Anything else Lima reports, such as `Broken`.
  Broken,
}

impl Status {
  /// The state `status` reports.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Status::Running => "running",
      Status::Stopped => "stopped",
      Status::Broken => "broken",
    }
  }
}

/// One VM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Instance {
  /// Its state.
  pub(crate) status: Status,
  /// The host directories it mounts.
  pub(crate) mounts: Vec<PathBuf>,
}

/// The major version in `limactl version 2.2.1`.
pub(crate) fn major_version(text: &str) -> Option<u32> {
  let version = text.trim().rsplit(' ').next()?;
  version
    .trim_start_matches('v')
    .split('.')
    .next()?
    .parse()
    .ok()
}

/// The instance called `name` in `limactl list --format json` output.
///
/// # Errors
/// A line that is not a JSON object.
pub(crate) fn find(text: &str, name: &str) -> Result<Option<Instance>, String> {
  for line in text.lines().filter(|line| !line.trim().is_empty()) {
    let doc: Value = serde_json::from_str(line)
      .map_err(|err| format!("limactl list printed a line that is not JSON: {err}"))?;
    if doc.get("name").and_then(Value::as_str) == Some(name) {
      return Ok(Some(instance(&doc)));
    }
  }
  Ok(None)
}

fn instance(doc: &Value) -> Instance {
  let status = match doc.get("status").and_then(Value::as_str) {
    Some("Running") => Status::Running,
    Some("Stopped") => Status::Stopped,
    _ => Status::Broken,
  };
  let mounts = doc
    .get("config")
    .and_then(|config| config.get("mounts"))
    .and_then(Value::as_array)
    .map(|mounts| {
      mounts
        .iter()
        .filter_map(|mount| mount.get("location").and_then(Value::as_str))
        .map(PathBuf::from)
        .collect()
    })
    .unwrap_or_default();
  Instance { status, mounts }
}

#[cfg(test)]
#[path = "tests/lima_instance_test.rs"]
mod tests;
