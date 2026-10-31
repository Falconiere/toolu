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

use super::context::Callers;
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
  /// The masked `HEAD` lines naming the changed definitions, at levels that send them.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub context: Option<String>,
}

impl Package {
  /// The text a dismissal quote must come from.
  pub(crate) fn evidence(&self) -> String {
    [
      Some(&self.patch),
      self.source.as_ref(),
      self.context.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(String::as_str)
    .collect::<Vec<_>>()
    .join("\n")
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
  let mut callers = Callers::new(budget.context_bytes);
  let clean = |text: String| mask.apply(&redact_text(&text));
  let packages = shaped
    .files
    .iter()
    .enumerate()
    .map(|(index, file)| {
      let text = if chunked.contains(&index) {
        head((env, root), file, &mut notes)
      } else {
        None
      };
      let context = text
        .as_deref()
        .filter(|_| budget.context_bytes > 0)
        .and_then(|text| callers.of((env, root), shaped, (file, text), &mut notes));
      let source = text.and_then(|text| surrounding(file, text, budget, &mut used, &mut notes));
      Package {
        path: file.path.clone(),
        patch: mask.apply(&file.patch),
        source: source.map(clean),
        context: context.map(clean),
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

/// `file`'s `HEAD` text when its kind needs context; a note when unreadable.
fn head((env, root): (&Env, &Path), file: &ShapedFile, notes: &mut Vec<String>) -> Option<String> {
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
  if text.is_none() {
    notes.push(format!("source of {path} left out: unreadable at HEAD"));
  }
  text
}

/// `text`, or windows of it, when it fits both the file and the package cap.
fn surrounding<'a>(
  file: &'a ShapedFile,
  text: String,
  budget: &Budget,
  used: &mut BTreeMap<&'a str, usize>,
  notes: &mut Vec<String>,
) -> Option<String> {
  let path = &file.path;
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

#[cfg(test)]
#[path = "tests/callers_test.rs"]
mod callers_tests;
