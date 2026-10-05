//! Which command upgrades the running `toolu`, judged from where it is installed.

use std::path::Path;

use toolu_protocol::install::{BREW_UPGRADE, INSTALLER};

/// Homebrew prefixes whose binaries `brew upgrade` owns. `/usr/local` is not one:
/// it is also the installer's directory, and an Intel Homebrew link there
/// resolves into `/usr/local/Cellar`.
const BREW_PREFIXES: &[&str] = &["/opt/homebrew/", "/home/linuxbrew/.linuxbrew/"];

/// `brew upgrade toolu` for a Homebrew install, else the installer command.
/// `exe` is canonicalized first, so a `bin/toolu` link into a `Cellar` counts.
pub fn upgrade_command(exe: &Path) -> &'static str {
  let resolved = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
  let text = resolved.to_string_lossy();
  let in_cellar = resolved
    .components()
    .any(|part| part.as_os_str() == "Cellar");
  if in_cellar || BREW_PREFIXES.iter().any(|prefix| text.starts_with(prefix)) {
    BREW_UPGRADE
  } else {
    INSTALLER
  }
}

#[cfg(test)]
#[path = "tests/install_test.rs"]
mod tests;
