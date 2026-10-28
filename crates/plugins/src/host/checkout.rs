//! The toolu checkout Muse installs from (#549, the D-localdir design of
//! `docs/research/muse-plugin-distribution.md`): a shallow clone of the release
//! tag that matches this binary, registered with Muse as the marketplace
//! `toolu`. Muse records the directory itself, so moving the checkout to a new
//! tag and refreshing the marketplace is how plugins update in place.

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;

use crate::fail::{Fail, Res};

/// Where releases come from.
pub(crate) const ORIGIN: &str = "https://github.com/Falconiere/toolu.git";

/// The catalog Muse reads from a toolu checkout; its presence marks one.
pub(crate) const CATALOG: &str = ".agents/plugins/marketplace.json";

/// The checkout directory of Muse under the data home.
const SUBDIR: &str = "toolu/muse-marketplace";

/// The checkout directory of Cursor's installer under the data home (#589).
pub(crate) const CURSOR_SUBDIR: &str = "toolu/cursor-marketplace";

/// The checkout directory of `OpenCode`'s installer under the data home.
pub(crate) const OPENCODE_SUBDIR: &str = "toolu/opencode-marketplace";

/// The checkout of one release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Checkout {
  /// `$XDG_DATA_HOME/toolu/muse-marketplace`.
  pub(crate) dir: PathBuf,
  /// `v<binary version>`.
  pub(crate) tag: String,
  /// The repository it is cloned from.
  pub(crate) origin: String,
}

impl Checkout {
  /// The checkout for a binary at `version`: under an absolute
  /// `XDG_DATA_HOME` (a relative one is ignored, as the XDG spec says), else
  /// under `$HOME/.local/share`.
  pub(crate) fn locate(env: &Env, version: &str, origin: &str) -> Res<Checkout> {
    Checkout::locate_in(env, version, origin, SUBDIR)
  }

  /// [`Checkout::locate`] for the data-home directory `subdir`.
  pub(crate) fn locate_in(env: &Env, version: &str, origin: &str, subdir: &str) -> Res<Checkout> {
    let xdg = env
      .get("XDG_DATA_HOME")
      .map(PathBuf::from)
      .filter(|dir| dir.is_absolute());
    let home = env
      .get("HOME")
      .map(|home| Path::new(home).join(".local/share"));
    let Some(data) = xdg.or(home) else {
      return Err(Fail::failed(
        "cannot place the marketplace checkout: set XDG_DATA_HOME or HOME",
      ));
    };
    Ok(Checkout {
      dir: data.join(subdir),
      tag: format!("v{version}"),
      origin: origin.to_owned(),
    })
  }

  /// The directory as text, for argv.
  pub(crate) fn dir_text(&self) -> String {
    self.dir.display().to_string()
  }

  fn git(&self, words: &[&str]) -> Vec<String> {
    let dir = self.dir_text();
    ["git", "-C", &dir]
      .into_iter()
      .chain(words.iter().copied())
      .map(str::to_owned)
      .collect()
  }

  /// A shallow clone of the tag.
  pub(crate) fn clone_argv(&self) -> Vec<String> {
    [
      "git",
      "clone",
      "--quiet",
      "--depth",
      "1",
      "--branch",
      &self.tag,
      &self.origin,
      &self.dir_text(),
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
  }

  /// Prints the checkout's origin URL.
  pub(crate) fn origin_argv(&self) -> Vec<String> {
    self.git(&["remote", "get-url", "origin"])
  }

  /// Succeeds when the tag is already fetched.
  pub(crate) fn has_tag_argv(&self) -> Vec<String> {
    let reference = format!("refs/tags/{}", self.tag);
    self.git(&["rev-parse", "--quiet", "--verify", &reference])
  }

  /// Fetches the tag alone.
  pub(crate) fn fetch_argv(&self) -> Vec<String> {
    self.git(&[
      "fetch", "--quiet", "--depth", "1", "origin", "tag", &self.tag,
    ])
  }

  /// Moves the checkout to the tag.
  pub(crate) fn detach_argv(&self) -> Vec<String> {
    self.git(&["checkout", "--quiet", "--detach", &self.tag])
  }

  /// Whether `path` is the checkout directory, as written or resolved.
  pub(crate) fn is(&self, path: &str) -> bool {
    self.spellings().iter().any(|dir| Path::new(path) == dir)
  }

  /// Whether `path` lies inside the checkout, as written or resolved: a plugin
  /// Muse installed from anywhere else is not toolu's to change.
  pub(crate) fn owns(&self, path: &str) -> bool {
    self
      .spellings()
      .iter()
      .any(|dir| Path::new(path).starts_with(dir))
  }

  fn spellings(&self) -> Vec<PathBuf> {
    let mut spellings = vec![self.dir.clone()];
    if let Ok(resolved) = std::fs::canonicalize(&self.dir) {
      spellings.push(resolved);
    }
    spellings
  }
}

#[cfg(test)]
#[path = "tests/checkout_test.rs"]
mod tests;
