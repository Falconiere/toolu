//! The hosts toolu runs under, named the way `toolu --host` takes them.

/// An agent host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
  /// Claude Code.
  Claude,
  /// Codex.
  Codex,
  /// `OpenCode`.
  Opencode,
  /// Cursor.
  Cursor,
  /// Hermes.
  Hermes,
}

impl Host {
  /// Every host, in `--host` order.
  pub const ALL: [Host; 5] = [
    Host::Claude,
    Host::Codex,
    Host::Opencode,
    Host::Cursor,
    Host::Hermes,
  ];

  /// The `--host` value.
  pub fn name(self) -> &'static str {
    match self {
      Host::Claude => "claude",
      Host::Codex => "codex",
      Host::Opencode => "opencode",
      Host::Cursor => "cursor",
      Host::Hermes => "hermes",
    }
  }

  /// The host a `--host` value names; exact lower-case names only.
  pub fn parse(text: &str) -> Option<Host> {
    Host::ALL.into_iter().find(|host| host.name() == text)
  }
}

#[cfg(test)]
#[path = "tests/host_test.rs"]
mod tests;
