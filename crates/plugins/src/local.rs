//! The hosts toolu installs into by copying plugin directories: Cursor (#589)
//! and `OpenCode`. Neither has a plugin command to drive. `cursor-agent plugin`
//! manages only marketplaces, and an interactive `/plugin` records installs
//! account-side; `OpenCode`'s `opencode plugin add` installs npm packages. Both
//! load any plugin directory in one local directory: Cursor
//! `~/.cursor/plugins/local`, `OpenCode` `<config>/plugins`. These verbs clone
//! toolu's release tag (the Muse checkout design, in each host's own
//! directory) and copy plugin directories from it. `OpenCode` loads a directory's
//! `server.js`, so each `OpenCode` copy also gets the generated shim.

use crate::args::Approval;
use crate::fail::Res;
use crate::host::Target;
use crate::host::adapter::Adapter;
use crate::host::checkout::{CURSOR_SUBDIR, Checkout, OPENCODE_SUBDIR};
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

/// One run on a local-directory host: the directory and the checkout it copies from.
pub(crate) struct Local {
  target: Target,
  store: Store,
  checkout: Checkout,
  git: Driver,
}

impl Local {
  /// The setup of `target` for a binary at `cx.version` cloning from `cx.origin`.
  pub(crate) fn new(cx: &crate::dispatch::Context<'_>, target: Target) -> Res<Local> {
    let subdir = if target == Target::Opencode {
      OPENCODE_SUBDIR
    } else {
      CURSOR_SUBDIR
    };
    let checkout = Checkout::locate_in(&cx.env, cx.version, &cx.origin, subdir)?;
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
    Ok(Local {
      target,
      store: Store::locate(&cx.env, target),
      checkout,
      git,
    })
  }

  /// The `--host` value, for messages.
  fn host(&self) -> &'static str {
    self.target.name()
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
#[path = "tests/local_test.rs"]
mod tests;
