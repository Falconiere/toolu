//! Push-review waivers (`packages/toolu-core/src/ledger/push-waiver.ts`): "yes,
//! push anyway", remembered for exactly one diff. The push-review gate records
//! a pending marker naming the diff sha ([`Waivers::pend`]); a successful push
//! of that sha promotes it to a waiver ([`Waivers::promote`]); the next push of
//! that diff finds it ([`Waivers::matches`]). A new commit changes the sha, so
//! the waiver stops matching. Files are atomic, 0600, and `jq -c` lines.

use std::path::Path;
use std::time::SystemTime;

use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::io::write_atomic;
use toolu_state::time::iso_seconds;

use crate::ledger::jq::{alt, get, number, parse_json, raw, string};

/// The waiver file version.
pub const PUSH_WAIVER_VERSION: u32 = 1;

/// One branch's waiver files.
#[derive(Debug, Clone, Copy)]
pub struct Waivers<'a> {
  /// The environment and host.
  pub roots: &'a Roots,
  /// The project root; the host's project when `None`.
  pub root: Option<&'a Path>,
  /// The branch slug.
  pub slug: &'a str,
}

/// A waiver-shaped file as an object, when it is one.
fn read_object(file: &str) -> Option<Ordered> {
  if !std::fs::metadata(file).is_ok_and(|meta| meta.is_file()) {
    return None;
  }
  let text = std::fs::read(file).ok()?;
  parse_json(&String::from_utf8_lossy(&text)).filter(|doc| matches!(doc, Ordered::Object(_)))
}

/// The diff sha of a v1 waiver-shaped file; a missing or unreadable file, an
/// unknown version and an empty sha all read as no waiver, never a match.
fn waiver_sha(file: &str) -> Option<String> {
  let doc = read_object(file)?;
  let empty = string("");
  let version = raw(alt(get(&doc, "version").ok()?, &empty));
  if version != PUSH_WAIVER_VERSION.to_string() {
    return None;
  }
  let sha = raw(alt(get(&doc, "diff_sha").ok()?, &empty));
  (!sha.is_empty()).then_some(sha)
}

fn write(file: &str, doc: &Ordered) -> bool {
  let path = Path::new(file);
  if let Some(dir) = path.parent()
    && std::fs::create_dir_all(dir).is_err()
  {
    return false;
  }
  write_atomic(path, &format!("{}\n", jq_text(doc, false)))
}

impl Waivers<'_> {
  /// `push_waiver_dir ROOT`: `$STATE_DIR`, else the project's push-review state dir.
  pub fn dir(&self) -> String {
    if let Some(dir) = self.roots.env().get("STATE_DIR") {
      return dir.to_owned();
    }
    self
      .roots
      .project_state_dir("push-review", None, self.root)
      .ok()
      .flatten()
      .map(|dir| dir.display().to_string())
      .unwrap_or_default()
  }

  /// `<dir>/<slug>.waiver.json`.
  pub fn path(&self) -> String {
    format!("{}/{}.waiver.json", self.dir(), self.slug)
  }

  /// `<dir>/<slug>.pending-waiver.json`.
  pub fn pending_path(&self) -> String {
    format!("{}/{}.pending-waiver.json", self.dir(), self.slug)
  }

  /// `push_waiver_matches ROOT SLUG SHA`: a waiver covers exactly `sha`.
  pub fn matches(&self, sha: &str) -> bool {
    !sha.is_empty() && waiver_sha(&self.path()).as_deref() == Some(sha)
  }

  /// `push_waiver_pend ROOT SLUG SHA BASE REASON_CODE`: record the question; the latest wins.
  pub fn pend(&self, sha: &str, base: &str, reason_code: &str, now: SystemTime) -> bool {
    if sha.is_empty() {
      return false;
    }
    let doc = Ordered::Object(vec![
      ("version".to_owned(), number(f64::from(PUSH_WAIVER_VERSION))),
      ("branch".to_owned(), string(self.slug)),
      ("diff_sha".to_owned(), string(sha)),
      ("base_branch".to_owned(), string(base)),
      ("reason_code".to_owned(), string(reason_code)),
      ("asked_at".to_owned(), string(&iso_seconds(now))),
    ]);
    write(&self.pending_path(), &doc)
  }

  /// `push_waiver_promote ROOT SLUG SHA`: cash in the pending marker, only when
  /// it names `sha`; a marker from an older diff must not waive different code.
  pub fn promote(&self, sha: &str, now: SystemTime) -> bool {
    if sha.is_empty() {
      return false;
    }
    let pending = self.pending_path();
    if waiver_sha(&pending).as_deref() != Some(sha) {
      return false;
    }
    let Some(Ordered::Object(mut entries)) = read_object(&pending) else {
      return false;
    };
    entries.retain(|(key, _)| key != "asked_at");
    let mut waiver = Ordered::Object(entries);
    waiver.set("waived_at", string(&iso_seconds(now)));
    if !write(&self.path(), &waiver) {
      return false;
    }
    let _removed = std::fs::remove_file(&pending);
    true
  }
}

#[cfg(test)]
#[path = "tests/waiver_test.rs"]
mod tests;
