//! How the `toolu` binary is installed and upgraded (#411), and the one-line
//! messages the launcher prints when it is missing.
//!
//! Every message is cmd-safe: none of `( ) & < > ^ % " '`, and no ` hook `, so it
//! fits inside sh single quotes, a cmd `echo` and a JSON string. The one `|`
//! (in the installer command) is written `^|` for cmd.

/// The installer one-liner (`get.toolu.sh`).
pub const INSTALLER: &str = "curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash";

/// The Homebrew install command.
pub const BREW_INSTALL: &str = "brew install falconiere/tap/toolu";

/// The Homebrew upgrade command.
pub const BREW_UPGRADE: &str = "brew upgrade toolu";

/// What a plugin hook says when no native `toolu` was found.
pub fn missing_message(plugin: &str) -> String {
  format!(
    "{plugin} plugin: no usable native toolu - checked TOOLU_BIN, /opt/homebrew/bin, \
     /usr/local/bin, /home/linuxbrew/.linuxbrew/bin, ~/.local/bin and PATH. If TOOLU_BIN is set, \
     it must point to a native toolu and is never replaced. Install toolu with: {INSTALLER}, \
     or: {BREW_INSTALL}. Then restart the session. See docs/install.md."
  )
}

#[cfg(test)]
#[path = "tests/install_test.rs"]
mod tests;
