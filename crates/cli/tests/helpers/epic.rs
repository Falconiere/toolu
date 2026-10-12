//! Running `toolu epic` against a temp home, resource root and epic state.

use std::path::PathBuf;
use std::process::{Command, Output};

/// A fallible helper result.
pub(crate) type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// The `toolu` binary under test.
pub(crate) const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// A temp root for one test: home, resources and the epic state.
pub(crate) struct Home {
  pub dir: tempfile::TempDir,
}

impl Home {
  /// A fresh root.
  pub(crate) fn new() -> Res<Self> {
    Ok(Self {
      dir: tempfile::tempdir()?,
    })
  }

  /// A path under the root.
  pub(crate) fn path(&self, rel: &str) -> PathBuf {
    self.dir.path().join(rel)
  }

  /// Run `toolu` with `args` in a clean environment rooted here.
  pub(crate) fn run(&self, args: &[&str]) -> Res<Output> {
    let mut command = Command::new(TOOLU);
    command
      .env_clear()
      .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
      .env("HOME", self.path("home"))
      .env("TOOLU_RESOURCE_HOME", self.path("resources"))
      .env("EPIC_STATE_HOME", self.path("epics"))
      .env("HERDR_SOCKET_PATH", self.path("no-herdr.sock"))
      .env("TOOLU_EPIC_HERDR", "off")
      .args(args);
    // Coverage runs write each child's profile where the harness says.
    if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
    Ok(command.output()?)
  }
}

/// Stderr as text.
pub(crate) fn err(output: &Output) -> String {
  String::from_utf8_lossy(&output.stderr).into_owned()
}
