//! Plugin install records as session-start saw them (`plugin-presence.ts`).
//! Codex reads its snapshot; Cursor and Hermes use Claude's install file.
//! Muse and `OpenCode` have no shared install record, so their presence is unknown here.

use toolu_engine::registry::gate::plugin_presence;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::host::snapshot::Installed;

/// `detect_plugin_installed`. Cursor and Hermes use Claude's record.
pub(crate) fn presence(spec: &str, env: &Env, host: Host) -> Installed {
  let mapped = match host {
    Host::Cursor | Host::Hermes => Host::Claude,
    Host::Claude | Host::Codex => host,
    // Neither keeps an install record the engine reads: unknown runs the rules.
    Host::Muse | Host::Opencode => {
      return if spec.is_empty() {
        Installed::Absent
      } else {
        Installed::Unknown
      };
    }
  };
  plugin_presence(spec, &Roots::new(env.clone(), Some(mapped)))
}

/// Installed, or indeterminate (fail open).
pub(crate) fn plugin_active(spec: &str, env: &Env, host: Host) -> bool {
  presence(spec, env, host) != Installed::Absent
}

/// The host's command to install a missing plugin.
pub(crate) fn install_command(spec: &str, host: Host) -> String {
  match host {
    Host::Codex => format!("codex plugin add {spec}"),
    Host::Muse => format!("muse plugins install {spec}"),
    Host::Opencode => format!(
      "toolu plugins install {} --host opencode",
      spec.split('@').next().unwrap_or(spec)
    ),
    Host::Claude | Host::Cursor | Host::Hermes => format!("/plugin install {spec}"),
  }
}

#[cfg(test)]
#[path = "tests/presence_test.rs"]
mod tests;
