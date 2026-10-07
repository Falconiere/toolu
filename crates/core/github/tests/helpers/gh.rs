//! A real `gh` reading a temporary config directory, so its token can be set,
//! changed or broken between calls.

use std::fs;
use std::io;

use tempfile::TempDir;
use toolu_runtime::env::Env;

/// A `gh` config directory.
pub(crate) struct Gh {
  dir: TempDir,
}

impl Gh {
  /// An empty config directory.
  pub(crate) fn new() -> io::Result<Gh> {
    Ok(Gh {
      dir: tempfile::tempdir()?,
    })
  }

  /// `hosts.yml` holding `token` for github.com, in gh's single-account shape.
  pub(crate) fn token(&self, token: &str) -> io::Result<()> {
    let hosts = format!(
      "github.com:\n    oauth_token: {token}\n    user: fixture\n    git_protocol: https\n"
    );
    fs::write(self.dir.path().join("hosts.yml"), hosts)
  }

  /// `hosts.yml` that is not YAML, holding `text`: `gh auth token` fails on it.
  pub(crate) fn broken(&self, text: &str) -> io::Result<()> {
    fs::write(
      self.dir.path().join("hosts.yml"),
      format!("github.com: [{text}\n"),
    )
  }

  /// The environment for `gh`: this process's `PATH` and the config directory
  /// as `HOME` and `GH_CONFIG_DIR`, without `GH_TOKEN`.
  pub(crate) fn env(&self) -> Env {
    let dir = self.dir.path().to_string_lossy().into_owned();
    Env::from_pairs([
      ("PATH", std::env::var("PATH").unwrap_or_default()),
      ("HOME", dir.clone()),
      ("GH_CONFIG_DIR", dir),
    ])
  }
}
