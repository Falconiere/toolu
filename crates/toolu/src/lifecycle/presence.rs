//! Plugin install records as session-start saw them (`plugin-presence.ts`).
//! Codex reads its snapshot. Every other host reads Claude's install file.

use toolu_engine::registry::gate::plugin_presence;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::host::snapshot::Installed;

/// `detect_plugin_installed`. Cursor, `OpenCode` and Hermes use Claude's record.
pub(crate) fn presence(spec: &str, env: &Env, host: Host) -> Installed {
  let mapped = if host == Host::Codex {
    Host::Codex
  } else {
    Host::Claude
  };
  plugin_presence(spec, &Roots::new(env.clone(), Some(mapped)))
}

/// Installed, or indeterminate (fail open).
pub(crate) fn plugin_active(spec: &str, env: &Env, host: Host) -> bool {
  presence(spec, env, host) != Installed::Absent
}

/// Codex's CLI, otherwise Claude Code's slash command.
pub(crate) fn install_command(spec: &str, host: Host) -> String {
  if host == Host::Codex {
    format!("codex plugin add {spec}")
  } else {
    format!("/plugin install {spec}")
  }
}

#[cfg(test)]
#[path = "tests/presence_test.rs"]
mod tests;
