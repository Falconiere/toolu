//! The hosts toolu runs under, named the way `toolu --host` takes them.

/// An agent host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
  /// Claude Code.
  Claude,
  /// Codex.
  Codex,
  /// Cursor.
  Cursor,
  /// Hermes.
  Hermes,
  /// Muse Code.
  Muse,
}

impl Host {
  /// Every host, in `--host` order.
  pub const ALL: [Host; 5] = [
    Host::Claude,
    Host::Codex,
    Host::Cursor,
    Host::Hermes,
    Host::Muse,
  ];

  /// The `--host` value.
  pub fn name(self) -> &'static str {
    match self {
      Host::Claude => "claude",
      Host::Codex => "codex",
      Host::Cursor => "cursor",
      Host::Hermes => "hermes",
      Host::Muse => "muse",
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
