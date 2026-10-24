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

/// Codex 0.164.0's refusal to add a git source that differs from the
/// registered one: the only add failure a remove can fix.
const REFUSED: &str = "already added from a different source";

/// What pinning did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pin {
  /// The marketplace is pinned to the tag: added now, or already.
  Pinned,
  /// toolu's own marketplace followed another ref and now follows the tag;
  /// the installed plugins still hold the old content.
  Repinned,
  /// Left as it was (a source other than toolu's, or none that could be
  /// added); the text is the hint for a plugin that then fails.
  Unpinned(String),
}

/// Pins the Codex marketplace `toolu` to `tag`. A `toolu` marketplace from
/// anywhere else is left alone, and is checked first: Codex replaces a local
/// one with a git add. Codex refuses to add a git source that differs from the
/// registered one, so toolu's own marketplace on another ref is removed and
/// added again pinned; Codex keeps its installed plugins and their hook trust
/// across that.
///
/// # Errors
/// Codex's marketplace list cannot be read, the pinned add of toolu's own
/// marketplace fails for another reason than that refusal, the remove fails,
/// or the pinned add fails after the remove (the unpinned marketplace is added
/// back first).
pub(crate) fn ensure(driver: &Driver, tag: &str) -> Res<Pin> {
  let list = codex::marketplace_list();
  let listing = driver.run(&list);
  if !listing.ok() {
    let why = first_line(&listing.stderr, &format!("exited {}", listing.code));
    return Err(cannot(tag, &format!("{}: {why}", list.join(" "))));
  }
  let source = codex::marketplace_source(&listing.stdout, MARKETPLACE)
    .map_err(|detail| cannot(tag, &format!("{}: {detail}", list.join(" "))))?;
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
  let why = first_line(&ran.stderr, "no output");
  match source {
    None => Ok(Pin::Unpinned(format!(
      "; the marketplace add also failed: {why}"
    ))),
    Some(_ours) if ran.stderr.contains(REFUSED) => repin(driver, &pinned, tag),
    Some(_ours) => Err(cannot(tag, &why)),
  }
}

fn cannot(tag: &str, why: &str) -> Fail {
  Fail::failed(format!(
    "cannot pin the Codex marketplace {MARKETPLACE} to {tag}: {why}"
  ))
}

/// Whether `source` is toolu's own repository, however Codex spelled it.
fn ours(source: &codex::Source) -> bool {
  let bare = |url: &str| {
    url
      .trim_end_matches('/')
      .trim_end_matches(".git")
      .to_ascii_lowercase()
  };
  source.kind == "git" && bare(&source.source) == bare(ORIGIN)
}

fn repin(driver: &Driver, pinned: &[String], tag: &str) -> Res<Pin> {
  let removed = driver.run(&codex::marketplace_remove());
  if !removed.ok() {
    return Err(cannot(
      tag,
      &first_line(&removed.stderr, "the remove failed"),
    ));
  }
  let added = driver.run(pinned);
  if added.ok() {
    return Ok(Pin::Repinned);
  }
  let why = first_line(&added.stderr, "the add failed");
  let unpinned = driver.adapter.add_marketplace(None);
  let restored = driver.run(&unpinned);
  if restored.ok() {
    return Err(cannot(tag, &format!("{why}; restored it unpinned")));
  }
  Err(cannot(
    tag,
    &format!(
      "{why}; restoring it unpinned failed too: {}; run `{}`",
      first_line(&restored.stderr, "no output"),
      unpinned.join(" ")
    ),
  ))
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

#[cfg(test)]
#[path = "tests/pin_failure_test.rs"]
mod failure_tests;
