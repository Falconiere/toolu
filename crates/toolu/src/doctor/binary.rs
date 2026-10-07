//! The running binary and the non-login shell probe (#443).

use std::path::PathBuf;

use serde_json::json;
use toolu_protocol::HOOK_PROTOCOL;
use toolu_protocol::install::BREW_UPGRADE;
use toolu_runtime::install::upgrade_command;
use toolu_runtime::process::commands::{ShellToolu, native_toolu_on_path};

use super::checks::{Check, Status};

/// What the non-login shell resolved.
pub(super) struct Probe {
  /// Canonical path of a native binary, when the probe proved one.
  native: Option<PathBuf>,
  /// A resolved path whose protocol is not a positive integer.
  listed: Option<PathBuf>,
  /// `command -v` printed a path that is not native toolu.
  shadowed: bool,
  /// The shell or the probe could not be run.
  error: Option<String>,
}

impl Probe {
  fn missed() -> Probe {
    Probe {
      native: None,
      listed: None,
      shadowed: false,
      error: None,
    }
  }

  fn at(native: PathBuf) -> Probe {
    Probe {
      native: Some(native),
      listed: None,
      shadowed: false,
      error: None,
    }
  }
}

/// Resolve `toolu` the way an agent's command shell does, once.
pub(super) fn probe() -> Probe {
  match native_toolu_on_path() {
    Ok(ShellToolu::Native(path)) => Probe::at(path),
    Ok(ShellToolu::Missing) => Probe::missed(),
    Ok(ShellToolu::Shadowed(path)) => Probe {
      listed: Some(path),
      shadowed: true,
      ..Probe::missed()
    },
    Err(error) => Probe {
      error: Some(error),
      ..Probe::missed()
    },
  }
}

/// Version, protocol, path and the upgrade command. Always ok.
pub(super) fn binary(probe: &Probe) -> Check {
  let path = probe
    .native
    .clone()
    .or_else(|| std::env::current_exe().ok())
    .unwrap_or_else(|| PathBuf::from("toolu"));
  let upgrade = upgrade_command(&path);
  let install = if upgrade == BREW_UPGRADE {
    "homebrew"
  } else {
    "installer"
  };
  Check::new(
    "binary",
    Status::Ok,
    format!(
      "toolu {} protocol {HOOK_PROTOCOL} at {}; upgrade with: {upgrade}",
      env!("CARGO_PKG_VERSION"),
      path.display()
    ),
    None,
    json!({
      "version": env!("CARGO_PKG_VERSION"),
      "hookProtocol": HOOK_PROTOCOL,
      "path": path.display().to_string(),
      "install": install,
      "upgrade": upgrade,
    }),
  )
}

/// The #443 reachability check. Not found fails; a non-native path is shadowed.
pub(super) fn reachability(probe: &Probe) -> Check {
  let (status, summary, reachable, path, shadowed) = reach_fields(probe);
  Check::new(
    "reachability",
    status,
    summary,
    None,
    json!({ "reachable": reachable, "path": path, "shadowed": shadowed }),
  )
}

fn reach_fields(probe: &Probe) -> (Status, String, bool, Option<String>, bool) {
  if let Some(error) = &probe.error {
    return (
      Status::Fail,
      error.clone(),
      false,
      probe.listed.as_ref().map(|path| path.display().to_string()),
      false,
    );
  }
  if let Some(path) = &probe.native {
    return (
      Status::Ok,
      format!("native toolu at {}", path.display()),
      true,
      Some(path.display().to_string()),
      false,
    );
  }
  if probe.shadowed {
    let path = probe.listed.as_ref().map(|path| path.display().to_string());
    let summary = match &path {
      Some(path) => format!("toolu on PATH is not the native binary ({path})"),
      None => "toolu on PATH is not the native binary".to_owned(),
    };
    return (Status::Fail, summary, false, path, true);
  }
  (
    Status::Fail,
    "the non-login shell does not resolve a native toolu".to_owned(),
    false,
    None,
    false,
  )
}

#[cfg(test)]
#[path = "tests/binary_test.rs"]
mod tests;
