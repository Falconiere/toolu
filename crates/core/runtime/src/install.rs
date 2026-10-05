//! Which command upgrades the running `toolu`, judged from where it is installed.

use std::path::Path;

use toolu_protocol::install::{BREW_UPGRADE, INSTALLER};

/// Homebrew prefixes whose binaries `brew upgrade` owns. `/usr/local` is not one:
/// it is also the installer's directory, and an Intel Homebrew link there
/// resolves into `/usr/local/Cellar`.
const BREW_PREFIXES: &[&str] = &["/opt/homebrew/", "/home/linuxbrew/.linuxbrew/"];

/// `brew upgrade toolu` for a Homebrew install, else the installer command.
/// `exe` is canonicalized first, so a `bin/toolu` link into a `Cellar` counts.
/// A path that does not resolve (a dangling link) is judged as given: the
/// caller passes the absolute `current_exe`, so a Homebrew prefix still matches
/// it, and this advice-only answer never needs to fail or write to stderr.
pub fn upgrade_command(exe: &Path) -> &'static str {
  let resolved = match std::fs::canonicalize(exe) {
    Ok(resolved) => resolved,
    Err(_unresolvable) => exe.to_path_buf(),
  };
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
