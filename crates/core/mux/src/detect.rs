//! Reachability probes for herdr and tmux.

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;
use toolu_state::detect::tools::tool_available;

/// Whether the herdr API socket is reachable for `env`.
#[must_use]
pub fn herdr_reachable(env: &Env) -> bool {
  socket_path(env).exists()
}

/// Where the herdr API socket lives for `env`.
#[must_use]
pub fn socket_path(env: &Env) -> PathBuf {
  if let Some(path) = env.get("HERDR_SOCKET_PATH") {
    return PathBuf::from(path);
  }
  let config = config_dir(env);
  match session_name(env) {
    Some(name) => config.join("sessions").join(name).join("herdr.sock"),
    None => config.join("herdr.sock"),
  }
}

/// The herdr session name from the env.
#[must_use]
pub fn session_name(env: &Env) -> Option<&str> {
  env
    .get("TOOLU_EPIC_HERDR_SESSION")
    .or_else(|| env.get("HERDR_SESSION"))
}

/// Whether a `tmux` binary is on `PATH`.
#[must_use]
pub fn tmux_available(env: &Env) -> bool {
  tool_available("tmux", env)
}

fn config_dir(env: &Env) -> PathBuf {
  match env.get("HERDR_CONFIG_PATH") {
    Some(path) => Path::new(path)
      .parent()
      .map_or_else(|| PathBuf::from("."), Path::to_path_buf),
    None => env.home().join(".config/herdr"),
  }
}

#[cfg(test)]
#[path = "tests/detect_test.rs"]
mod tests;
