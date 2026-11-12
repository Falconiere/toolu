//! Running `toolu epic` against a temp home, resource root and epic state.

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

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
    self.run_at(None, args).map(|(output, _)| output)
  }

  /// Run `toolu` from `cwd`, returning its output and launcher PID.
  pub(crate) fn run_in(&self, cwd: &std::path::Path, args: &[&str]) -> Res<(Output, u32)> {
    self.run_at(Some(cwd), args)
  }

  fn run_at(&self, cwd: Option<&std::path::Path>, args: &[&str]) -> Res<(Output, u32)> {
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
    if let Some(cwd) = cwd {
      command.current_dir(cwd);
    }
    // Coverage runs write each child's profile where the harness says.
    if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
    let child = command
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()?;
    let pid = child.id();
    Ok((child.wait_with_output()?, pid))
  }
}

/// Stderr as text.
pub(crate) fn err(output: &Output) -> String {
  String::from_utf8_lossy(&output.stderr).into_owned()
}
