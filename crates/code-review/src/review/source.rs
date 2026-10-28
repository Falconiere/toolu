//! The evidence package of each shaped file: its masked patch plus bounded
//! surrounding source from `HEAD`. Added files carry no source (their patch is the
//! whole file) and deleted files have none. A source over the level's per-file cap
//! sends the line windows around its changes that fit that cap instead, with a
//! note; one with no window that fits, or that would take its package (parent
//! directory) over the package cap, is left out whole with a note, never cut
//! mid-window. Source lines are outside the pre-pass scan, so the secret scanner
//! masks them here before they leave.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;
use toolu_runtime::env::Env;

use super::mask::Mask;
use super::window::windows;
use crate::budget::{BLOB_MAX_BYTES, Budget};
use crate::prepass::redact_text;
use crate::shape::git_io::git_process;
use crate::shape::{FileKind, Shaped, ShapedFile};

/// What a reviewer and Jev see of one file; every text is already masked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Package {
  /// The shaped file's path.
  pub path: String,
  /// The masked zero-context patch.
  pub patch: String,
  /// The masked `HEAD` source, when it fits the caps.
  pub source: Option<String>,
}

impl Package {
  /// The text a dismissal quote must come from.
  pub(crate) fn evidence(&self) -> String {
    match &self.source {
      Some(source) => format!("{}\n{source}", self.patch),
      None => self.patch.clone(),
    }
  }
}

/// One package per `shaped.files` entry, in order; a note for each source left out.
pub(crate) fn packages(
  env: &Env,
  root: &Path,
  shaped: &Shaped,
  budget: &Budget,
  mask: &Mask,
) -> (Vec<Package>, Vec<String>) {
  let mut used: BTreeMap<&str, usize> = BTreeMap::new();
  let mut notes = Vec::new();
  // Files a path scope left out of every chunk would only spend the package cap.
  let chunked: BTreeSet<usize> = shaped
    .chunks
    .iter()
    .flat_map(|chunk| chunk.files.iter().copied())
    .collect();
  let packages = shaped
    .files
    .iter()
    .enumerate()
    .map(|(index, file)| {
      let source = if chunked.contains(&index) {
        surrounding((env, root), file, budget, &mut used, &mut notes)
      } else {
        None
      };
      Package {
        path: file.path.clone(),
        patch: mask.apply(&file.patch),
        source: source.map(|text| mask.apply(&redact_text(&text))),
      }
    })
    .collect();
  (packages, notes)
}

/// `text` whole when it fits `cap`, else the windows around `file`'s changes
/// that fit, each with a note; `None` when not even one window fits.
fn fit(file: &ShapedFile, text: String, cap: usize, notes: &mut Vec<String>) -> Option<String> {
  if text.len() <= cap {
    return Some(text);
  }
  let path = &file.path;
  let (kept, sent, total) = windows(&text, file, cap);
  if sent == 0 {
    notes.push(format!("source of {path} left out: over {cap} bytes"));
    return None;
  }
  notes.push(format!(
    "source of {path}: {sent} of {total} windows around its changes sent, the whole file is over {cap} bytes"
  ));
  Some(kept)
}

/// `file`'s `HEAD` text, or windows of it, when its kind needs context and it
/// fits both caps.
fn surrounding<'a>(
  (env, root): (&Env, &Path),
  file: &'a ShapedFile,
  budget: &Budget,
  used: &mut BTreeMap<&'a str, usize>,
  notes: &mut Vec<String>,
) -> Option<String> {
  if matches!(file.kind, FileKind::Added | FileKind::Deleted) {
    return None;
  }
  let path = &file.path;
  let object = format!("HEAD:{path}");
  let read = git_process(env, root, &["cat-file", "blob", &object], BLOB_MAX_BYTES);
  let text = match read {
    Ok(out) if out.exit_code == 0 && !out.timed_out && !out.truncated => {
      String::from_utf8(out.stdout_bytes).ok()
    }
    Ok(out) if out.truncated => {
      notes.push(format!(
        "source of {path} left out: over {BLOB_MAX_BYTES} bytes"
      ));
      return None;
    }
    Ok(_) | Err(_) => None,
  };
  let Some(text) = text else {
    notes.push(format!("source of {path} left out: unreadable at HEAD"));
    return None;
  };
  let text = fit(file, text, budget.source_file_bytes, notes)?;
  let package = path.rsplit_once('/').map_or("", |(dir, _)| dir);
  let total = used.entry(package).or_insert(0);
  if *total + text.len() > budget.source_package_bytes {
    notes.push(format!(
      "source of {path} left out: package {package:?} over {} bytes",
      budget.source_package_bytes
    ));
    return None;
  }
  *total += text.len();
  Some(text)
}

#[cfg(test)]
#[path = "tests/source_test.rs"]
mod tests;
