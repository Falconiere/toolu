//! Push-review waivers (`ledger/push-waiver.ts`, a port of `push-waiver.sh`):
//! "yes, push anyway", remembered for exactly one diff. The push-review gate
//! records a pending marker naming the diff sha; a successful push of that same
//! sha promotes it to a waiver ([`push_waiver_promote`]). The bytes match the
//! TypeScript writer's `jq -c` lines, and writes are atomic and 0600.

use std::path::{Path, PathBuf};

use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

use crate::ctx::StateCtx;
use crate::io::write_atomic;
use crate::js_order::js_ordered;
use crate::time::iso_seconds;

/// The waiver file version this layer reads and writes.
const PUSH_WAIVER_VERSION: &str = "1";

/// `push_waiver_dir ROOT`: `$STATE_DIR`, else the push-review state dir of
/// `root` (the project root when `root` is empty).
pub fn push_waiver_dir(roots: &Roots, root: &Path) -> Option<PathBuf> {
  if let Some(dir) = roots.env().get("STATE_DIR") {
    return Some(PathBuf::from(dir));
  }
  let root = (!root.as_os_str().is_empty()).then_some(root);
  roots
    .project_state_root(None, root)
    .map(|base| base.join("push-review"))
}

/// `<dir>/<slug>.waiver.json`.
pub fn push_waiver_path(roots: &Roots, root: &Path, slug: &str) -> Option<PathBuf> {
  push_waiver_dir(roots, root).map(|dir| dir.join(format!("{slug}.waiver.json")))
}

/// `<dir>/<slug>.pending-waiver.json`.
pub fn push_waiver_pending_path(roots: &Roots, root: &Path, slug: &str) -> Option<PathBuf> {
  push_waiver_dir(roots, root).map(|dir| dir.join(format!("{slug}.pending-waiver.json")))
}

/// A regular file's JSON object, keys in `JSON.parse` order; `None` otherwise.
fn read_object(file: &Path) -> Option<Ordered> {
  if !std::fs::metadata(file).ok()?.is_file() {
    return None;
  }
  let bytes = std::fs::read(file).ok()?;
  match js_ordered(Ordered::parse(&String::from_utf8_lossy(&bytes)).ok()?) {
    object @ Ordered::Object(_) => Some(object),
    Ordered::Null
    | Ordered::Bool(_)
    | Ordered::Number(_)
    | Ordered::String(_)
    | Ordered::Array(_) => None,
  }
}

/// `jq -r '.<key> // ""'` of an object.
fn field(doc: &Ordered, key: &str) -> String {
  match doc.get(key) {
    None | Some(Ordered::Null | Ordered::Bool(false)) => String::new(),
    Some(Ordered::String(text)) => text.clone(),
    Some(value) => jq_text(value, true),
  }
}

/// `_push_waiver_sha`: the diff sha of a v1 waiver-shaped object; `None` for
/// another version or an empty sha, which must read as "no waiver".
fn waiver_sha(doc: &Ordered) -> Option<String> {
  if field(doc, "version") != PUSH_WAIVER_VERSION {
    return None;
  }
  Some(field(doc, "diff_sha")).filter(|sha| !sha.is_empty())
}

/// The object `doc` without `key`.
fn without(doc: Ordered, key: &str) -> Ordered {
  match doc {
    Ordered::Object(entries) => Ordered::Object(
      entries
        .into_iter()
        .filter(|(name, _)| name != key)
        .collect(),
    ),
    other @ (Ordered::Null
    | Ordered::Bool(_)
    | Ordered::Number(_)
    | Ordered::String(_)
    | Ordered::Array(_)) => other,
  }
}

/// `mkdir -p` the parent, then the `jq -c` line atomically.
fn write(file: &Path, doc: &Ordered) -> bool {
  let created = file
    .parent()
    .is_some_and(|dir| std::fs::create_dir_all(dir).is_ok());
  created && write_atomic(file, &format!("{}\n", jq_text(doc, false)))
}

/// `push_waiver_promote ROOT SLUG SHA`: cash in the pending marker, but only
/// when it names `sha`, so a marker from an older diff never waives other code.
/// The marker is removed once the waiver is written.
pub fn push_waiver_promote(ctx: &StateCtx, root: &Path, slug: &str, sha: &str) -> bool {
  if sha.is_empty() {
    return false;
  }
  let Some(pending) = push_waiver_pending_path(&ctx.roots, root, slug) else {
    return false;
  };
  let Some(marker) = read_object(&pending) else {
    return false;
  };
  if waiver_sha(&marker).as_deref() != Some(sha) {
    return false;
  }
  let mut waiver = without(marker, "asked_at");
  waiver.set("waived_at", Ordered::String(iso_seconds(ctx.now())));
  let written = push_waiver_path(&ctx.roots, root, slug).is_some_and(|path| write(&path, &waiver));
  if written {
    // `rm -f`: a marker already gone is the state we want.
    let _removed = std::fs::remove_file(&pending);
  }
  written
}

#[cfg(test)]
#[path = "tests/push_waiver_test.rs"]
mod tests;
