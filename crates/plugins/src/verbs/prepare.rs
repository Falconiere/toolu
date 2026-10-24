//! The marketplace a host installs from, made ready before `install` and
//! `update` touch a plugin. Claude Code adds the GitHub marketplace on install,
//! and a failure there is only a hint (it may be there already). Codex's is
//! pinned to this binary's tag on both verbs (#626). Muse needs toolu's
//! checkout at this binary's tag, registered as the marketplace `toolu`;
//! anything wrong there stops the run before any plugin command, so a failed
//! fetch leaves Muse untouched.

use std::io::ErrorKind;
use std::path::Path;

use crate::args::Verb;
use crate::fail::{Fail, Res};
use crate::host::adapter::{Adapter, MARKETPLACE};
use crate::host::checkout::{CATALOG, Checkout};
use crate::host::muse;
use crate::host::run::first_line;
use crate::verbs::Driver;
use crate::verbs::pin::{self, Pin};

/// The marketplace made ready.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Prepared {
  /// The hint for a plugin that then fails to install.
  pub(crate) hint: String,
  /// Codex's marketplace moved to the tag now, so every installed plugin is
  /// added again to take the tag's content.
  pub(crate) repinned: bool,
}

/// Makes the marketplace ready for `verb`.
///
/// # Errors
/// Muse's checkout or registration fails, or pinning Codex's marketplace
/// fails after removing it.
pub(crate) fn run(driver: &Driver, verb: Verb) -> Res<Prepared> {
  match (driver.adapter, &driver.checkout, &driver.pin) {
    (Adapter::Muse, Some(checkout), _pin) => {
      sync(driver, checkout)?;
      register(driver, checkout)?;
      Ok(Prepared::default())
    }
    (Adapter::Muse, None, _pin) => Err(Fail::failed("muse needs a marketplace checkout")),
    (Adapter::Codex, _checkout, Some(tag)) => Ok(match pin::ensure(driver, tag)? {
      Pin::Pinned => Prepared::default(),
      Pin::Repinned => Prepared {
        hint: String::new(),
        repinned: true,
      },
      Pin::Unpinned(hint) => Prepared {
        hint,
        repinned: false,
      },
    }),
    (Adapter::Claude | Adapter::Codex, _checkout, _pin) if verb == Verb::Install => Ok(Prepared {
      hint: add_marketplace(driver),
      repinned: false,
    }),
    (Adapter::Claude | Adapter::Codex, _checkout, _pin) => Ok(Prepared::default()),
  }
}

/// Adds the toolu marketplace. A failure is not fatal (it may be there
/// already), so it only becomes a hint on a plugin that then fails to install.
fn add_marketplace(driver: &Driver) -> String {
  let ran = driver.run(&driver.adapter.add_marketplace(None));
  if ran.ok() {
    return String::new();
  }
  format!(
    "; the marketplace add also failed: {}",
    first_line(&ran.stderr, "no output")
  )
}

/// Runs `argv`, failing the verb with its first stderr line.
fn must(driver: &Driver, argv: &[String]) -> Res<String> {
  let ran = driver.run(argv);
  if ran.ok() {
    return Ok(ran.stdout);
  }
  Err(Fail::failed(format!(
    "{}: {}",
    argv.join(" "),
    first_line(&ran.stderr, &format!("exited {}", ran.code))
  )))
}

/// Whether `path` is a regular file (`Some(false)` for anything else there),
/// or `None` when nothing is; an unreadable path is an error, never "absent".
fn kind(path: &Path) -> Res<Option<bool>> {
  match std::fs::metadata(path) {
    Ok(meta) => Ok(Some(meta.is_file())),
    Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
    Err(err) => Err(Fail::failed(format!(
      "cannot read {}: {err}",
      path.display()
    ))),
  }
}

/// Clones the tag, or moves an existing toolu checkout to it.
pub(crate) fn sync(driver: &Driver, checkout: &Checkout) -> Res<()> {
  let dir = checkout.dir_text();
  if kind(&checkout.dir)?.is_none() {
    if let Some(parent) = checkout.dir.parent() {
      std::fs::create_dir_all(parent)
        .map_err(|err| Fail::failed(format!("cannot create {}: {err}", parent.display())))?;
    }
    return must(driver, &checkout.clone_argv()).map(drop);
  }
  if kind(&checkout.dir.join(CATALOG))? != Some(true) {
    return Err(Fail::failed(format!(
      "{dir} is not a toolu checkout (it has no {CATALOG}); move it aside and run again"
    )));
  }
  let origin = must(driver, &checkout.origin_argv())?;
  if origin.trim() != checkout.origin {
    return Err(Fail::failed(format!(
      "{dir} is cloned from {}, not {}; move it aside and run again",
      origin.trim(),
      checkout.origin
    )));
  }
  if !driver.run(&checkout.has_tag_argv()).ok() {
    must(driver, &checkout.fetch_argv())?;
  }
  must(driver, &checkout.detach_argv()).map(drop)
}

/// Registers the checkout as the marketplace `toolu`, or refreshes its snapshot
/// when it already is; refuses a `toolu` marketplace from anywhere else.
fn register(driver: &Driver, checkout: &Checkout) -> Res<()> {
  let listing = must(driver, &muse::marketplace_list())?;
  let source = muse::marketplace(&listing, MARKETPLACE)
    .map_err(|detail| Fail::failed(format!("muse plugins marketplace list --json: {detail}")))?;
  match source {
    None => must(driver, &muse::marketplace_add(&checkout.dir_text())).map(drop),
    Some(source) if source.kind == "local-dir" && checkout.is(&source.path) => {
      must(driver, &muse::marketplace_update()).map(drop)
    }
    Some(source) => Err(Fail::failed(format!(
      "the Muse marketplace {MARKETPLACE} comes from {} ({}), not {}; run `muse plugins marketplace remove {MARKETPLACE}` to let toolu manage it",
      source.path,
      source.kind,
      checkout.dir_text()
    ))),
  }
}

#[cfg(test)]
#[path = "tests/prepare_test.rs"]
mod tests;
