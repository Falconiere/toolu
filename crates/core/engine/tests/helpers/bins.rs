//! Executables for tests: scripts, and where real tools live, so sentinel
//! wrappers can log a spawn and run the real program.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;

use super::sandbox::{Res, write};

/// An executable at `path` running `body` under `/bin/sh`.
pub(crate) fn script(path: &Path, body: &str) -> Res<()> {
  write(path, &format!("#!/bin/sh\n{body}\n"))?;
  let mode = std::fs::Permissions::from_mode(0o755);
  std::fs::set_permissions(path, mode).map_err(|err| err.to_string())
}

/// Where `name` is on the test process's `PATH`.
pub(crate) fn which(name: &str) -> Res<String> {
  let out = Command::new("sh")
    .args(["-c", &format!("command -v {name}")])
    .output()
    .map_err(|err| err.to_string())?;
  let path = String::from_utf8_lossy(&out.stdout).trim().to_owned();
  if path.is_empty() {
    return Err(format!("{name} is not on PATH"));
  }
  Ok(path)
}
