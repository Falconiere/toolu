//! The evidence package of each shaped file: its masked patch plus bounded
//! surrounding source from `HEAD`. Added files carry no source (their patch is the
//! whole file) and deleted files have none. A source over the level's per-file cap,
//! or one that would take its package (parent directory) over the package cap, is
//! left out whole with a note, never cut mid-file.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use toolu_runtime::env::Env;

use super::mask::Mask;
use crate::budget::Budget;
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
  let packages = shaped
    .files
    .iter()
    .map(|file| {
      let source = surrounding((env, root), file, budget, &mut used, &mut notes);
      Package {
        path: file.path.clone(),
        patch: mask.apply(&file.patch),
        source: source.map(|text| mask.apply(&text)),
      }
    })
    .collect();
  (packages, notes)
}

/// `file`'s `HEAD` text when its kind needs context and it fits both caps.
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
  let read = git_process(
    env,
    root,
    &["cat-file", "blob", &object],
    budget.source_file_bytes + 1,
  );
  let text = match read {
    Ok(out) if out.exit_code == 0 && !out.timed_out && !out.truncated => {
      String::from_utf8(out.stdout_bytes).ok()
    }
    Ok(out) if out.truncated => {
      notes.push(format!(
        "source of {path} left out: over {} bytes",
        budget.source_file_bytes
      ));
      return None;
    }
    Ok(_) | Err(_) => None,
  };
  let Some(text) = text else {
    notes.push(format!("source of {path} left out: unreadable at HEAD"));
    return None;
  };
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
