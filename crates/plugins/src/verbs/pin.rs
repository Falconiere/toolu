//! Codex's toolu marketplace pinned to this binary's release tag (#626).
//! At every session start Codex re-clones a git marketplace whose ref moved
//! and reinstalls its plugins, and a reinstalled plugin needs its hooks
//! reviewed again; an unpinned marketplace follows toolu's main, which moves
//! with every merge. A tag never moves.

use crate::fail::{Fail, Res};
use crate::host::adapter::MARKETPLACE;
use crate::host::checkout::ORIGIN;
use crate::host::codex;
use crate::host::run::first_line;
use crate::step::{Status, Step};
use crate::verbs::Driver;

/// What pinning did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pin {
  /// The marketplace is pinned to the tag: added now, or already.
  Pinned,
  /// toolu's own marketplace followed another ref and now follows the tag;
  /// the installed plugins still hold the old content.
  Repinned,
  /// Left as it was; the text is the hint for a plugin that then fails.
  Unpinned(String),
}

/// Pins the Codex marketplace `toolu` to `tag`. A `toolu` marketplace from
/// anywhere else is left alone, and is checked first: Codex replaces a local
/// one with a git add. Codex refuses to add a git source that differs from the
/// registered one, so toolu's own unpinned marketplace is removed and added
/// again pinned; Codex keeps its installed plugins and their hook trust
/// across that.
///
/// # Errors
/// The remove fails, or the pinned add fails after the remove (the unpinned
/// marketplace is added back first).
pub(crate) fn ensure(driver: &Driver, tag: &str) -> Res<Pin> {
  let listing = driver.run(&codex::marketplace_list());
  let source = if listing.ok() {
    codex::marketplace_source(&listing.stdout, MARKETPLACE)
      .ok()
      .flatten()
  } else {
    None
  };
  if let Some(source) = source.as_ref().filter(|source| !ours(source)) {
    return Ok(Pin::Unpinned(format!(
      "; the marketplace {MARKETPLACE} comes from {} ({}), so toolu left it unpinned",
      source.source, source.kind
    )));
  }
  let pinned = driver.adapter.add_marketplace(Some(tag));
  let ran = driver.run(&pinned);
  if ran.ok() {
    return Ok(Pin::Pinned);
  }
  if source.is_some() {
    return repin(driver, &pinned, tag);
  }
  Ok(Pin::Unpinned(format!(
    "; the marketplace add also failed: {}",
    first_line(&ran.stderr, "no output")
  )))
}

fn ours(source: &codex::Source) -> bool {
  source.kind == "git" && source.source == ORIGIN
}

fn repin(driver: &Driver, pinned: &[String], tag: &str) -> Res<Pin> {
  let removed = driver.run(&codex::marketplace_remove());
  if !removed.ok() {
    return Err(Fail::failed(format!(
      "cannot pin the Codex marketplace {MARKETPLACE} to {tag}: {}",
      first_line(&removed.stderr, "the remove failed")
    )));
  }
  let added = driver.run(pinned);
  if added.ok() {
    return Ok(Pin::Repinned);
  }
  let why = first_line(&added.stderr, "the add failed");
  let unpinned = driver.adapter.add_marketplace(None);
  let restored = driver.run(&unpinned);
  if restored.ok() {
    return Err(Fail::failed(format!(
      "cannot pin the Codex marketplace {MARKETPLACE} to {tag}: {why}; restored it unpinned"
    )));
  }
  Err(Fail::failed(format!(
    "cannot pin the Codex marketplace {MARKETPLACE} to {tag}: {why}; restoring it unpinned failed too: {}; run `{}`",
    first_line(&restored.stderr, "no output"),
    unpinned.join(" ")
  )))
}

/// Re-adds the installed plugin `name` after a re-pin, so its cached content
/// is the tag's.
pub(crate) fn refresh(driver: &Driver, name: &str) -> Step {
  let argv = driver.adapter.update(name);
  let ran = driver.run(&argv);
  if !ran.ok() {
    return Step::new(
      name,
      Status::Failed,
      first_line(&ran.stderr, "update failed"),
      argv,
    );
  }
  let tag = driver.pin.as_deref().unwrap_or("the release tag");
  Step::new(name, Status::Updated, format!("re-pinned to {tag}"), argv)
}

#[cfg(test)]
#[path = "tests/pin_test.rs"]
mod tests;
