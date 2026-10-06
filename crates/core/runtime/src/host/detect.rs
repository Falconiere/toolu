//! Which host spawned this hook (`packages/toolu-core/src/host/host-detect.ts`).

use toolu_protocol::host::Host;
use toolu_protocol::native::hosts_for_native;

use crate::env::Env;

/// The detected host, and the warning an invalid `TOOLU_HOST_OVERRIDE` leaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
  /// The host.
  pub host: Host,
  /// `toolu-host: invalid TOOLU_HOST_OVERRIDE '…' (using environment detection)`.
  pub warning: Option<String>,
}

/// Resolves the host, first hit wins: `TOOLU_HOST_OVERRIDE`, a hook event name
/// only one host uses, Cursor's per-hook variables, Codex's `PLUGIN_ROOT`, then
/// Claude. Hermes documents no hook environment, so only its event names (or
/// the override) select it.
pub fn detect(env: &Env, hook_event_name: Option<&str>) -> Detected {
  let mut warning = None;
  if let Some(name) = env.get("TOOLU_HOST_OVERRIDE") {
    if let Some(host) = Host::parse(name) {
      return Detected { host, warning };
    }
    warning = Some(format!(
      "toolu-host: invalid TOOLU_HOST_OVERRIDE '{name}' (using environment detection)"
    ));
  }
  let owners = hook_event_name
    .filter(|name| !name.is_empty())
    .map(hosts_for_native)
    .unwrap_or_default();
  let host = match owners.as_slice() {
    [owner] => *owner,
    _ if env
      .get("CURSOR_VERSION")
      .or(env.get("CURSOR_PROJECT_DIR"))
      .is_some() =>
    {
      Host::Cursor
    }
    _ if env.get("PLUGIN_ROOT").is_some() => Host::Codex,
    _ => Host::Claude,
  };
  Detected { host, warning }
}

#[cfg(test)]
#[path = "tests/detect_test.rs"]
mod tests;
