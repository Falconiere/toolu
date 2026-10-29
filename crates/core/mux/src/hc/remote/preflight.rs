//! Verify the worker command surface as well as its release version.

use super::{Error, Transfer, text};

impl Transfer<'_> {
  /// Verify checkout identity, Git and exact Toolu version, and the selected host CLI.
  ///
  /// # Errors
  /// A dependency is absent, versions differ, or the checkout origin differs.
  pub fn preflight(&self, origin: &str, toolu_version: &str, host: &str) -> Result<(), Error> {
    self.check()?;
    if !matches!(host, "claude" | "codex" | "muse" | "cursor" | "opencode") {
      return Err(Error::new("invalid_host", "unsupported host"));
    }
    self.git(&["--version"])?;
    self.git(&["rev-parse", "--show-toplevel"])?;
    if self.git(&["remote", "get-url", "origin"])? != origin {
      return Err(Error::new("identity_mismatch", "remote origin differs"));
    }
    if text(self.exec(&["toolu", "--version"], &[])?)? != toolu_version {
      return Err(Error::new(
        "version_mismatch",
        "remote Toolu version differs",
      ));
    }
    let epic = text(self.exec(&["toolu", "epic", "report", "--help"], &[])?)?;
    let fixer = text(self.exec(&["toolu", "babysit", "fixer-report", "--help"], &[])?)?;
    reports(&epic, &fixer)?;
    let binary = if host == "cursor" {
      "cursor-agent"
    } else {
      host
    };
    self.exec(&[binary, "--version"], &[])?;
    Ok(())
  }
}

fn reports(epic: &str, fixer: &str) -> Result<(), Error> {
  if !["--run ", "--expected-base "]
    .iter()
    .all(|flag| epic.contains(flag))
    || !["--run-id ", "--expected-base ", "--commit "]
      .iter()
      .all(|flag| fixer.contains(flag))
  {
    return Err(Error::new(
      "version_mismatch",
      "remote Toolu lacks bound worker reporting; update it explicitly before dispatch",
    ));
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/preflight_test.rs"]
mod tests;
