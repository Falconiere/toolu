//! Cursor as an install target (#589). `cursor-agent plugin` manages only
//! marketplaces, and an interactive `/plugin` records installs account-side, so
//! there is no command to drive. Cursor does load any plugin directory under
//! `~/.cursor/plugins/local`, so these verbs clone toolu's release tag (the Muse
//! checkout design, in its own directory) and copy plugin directories from it.

use crate::args::Approval;
use crate::fail::Res;
use crate::host::adapter::Adapter;
use crate::host::checkout::{CURSOR_SUBDIR, Checkout};
use crate::verbs::{Driver, prepare};

/// `install` and `update` on the local directory.
mod change;
/// `list` and `remove` on the local directory.
mod inspect;
/// The local plugins directory and its marker.
pub(crate) mod store;

pub(crate) use change::{install, update};
pub(crate) use inspect::{list, remove};
use store::Store;

/// One Cursor run: the local directory and the checkout it copies from.
pub(crate) struct Cursor {
  store: Store,
  checkout: Checkout,
  git: Driver,
}

impl Cursor {
  /// The setup for a binary at `version` cloning from `origin`.
  pub(crate) fn new(cx: &crate::dispatch::Context<'_>) -> Res<Cursor> {
    let checkout = Checkout::locate_in(&cx.env, cx.version, &cx.origin, CURSOR_SUBDIR)?;
    // Only `git` runs through this driver; the adapter is never asked for a plugin command.
    let git = Driver {
      adapter: Adapter::Claude,
      env: cx.env.clone(),
      timeout: cx.timeout,
      checkout: Some(checkout.clone()),
      pin: None,
      approval: Approval::Report,
      progress: None,
    };
    Ok(Cursor {
      store: Store::locate(&cx.env),
      checkout,
      git,
    })
  }

  /// Clones the tag, or moves an existing checkout to it.
  fn sync(&self) -> Res<()> {
    prepare::sync(&self.git, &self.checkout)
  }

  fn source(&self, name: &str) -> std::path::PathBuf {
    self.checkout.dir.join("plugins").join(name)
  }

  /// The command a copy stands for, as a dry run prints it.
  fn copy_argv(&self, name: &str) -> Vec<String> {
    ["cp", "-R"]
      .into_iter()
      .map(str::to_owned)
      .chain([
        self.source(name).display().to_string(),
        self.store.path(name).display().to_string(),
      ])
      .collect()
  }
}

#[cfg(test)]
#[path = "tests/cursor_test.rs"]
mod tests;
