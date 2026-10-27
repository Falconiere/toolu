//! The four agent CLIs `setup` points at but never installs: each vendor's own
//! install command and how to log in. Checked on Debian 13 arm64 (2026-10-27).

/// One agent CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Vendor {
  /// The command it installs.
  pub(crate) cli: &'static str,
  /// The vendor's install command.
  pub(crate) install: &'static str,
  /// How to log in once installed.
  pub(crate) login: &'static str,
}

/// Claude Code, Codex, Cursor and Muse Code.
pub(crate) const VENDORS: [Vendor; 4] = [
  Vendor {
    cli: "claude",
    install: "curl -fsSL https://claude.ai/install.sh | bash",
    login: "claude setup-token, or /login inside claude",
  },
  Vendor {
    cli: "codex",
    install: "npm install -g @openai/codex",
    login: "codex login",
  },
  Vendor {
    cli: "cursor-agent",
    install: "curl https://cursor.com/install -fsS | bash",
    login: "NO_OPEN_BROWSER=1 cursor-agent login",
  },
  Vendor {
    cli: "muse",
    install: "curl -fsSL https://dev.meta.ai/install.sh | bash",
    login: "muse login",
  },
];
