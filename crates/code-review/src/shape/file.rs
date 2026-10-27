//! One committed file's source and patch classification.

use super::git_io::git_output;
use super::parse::{Status, patch_lines};
use super::source::{batch_generated, old_side};
use super::{FileContext, FileKind, ShapeError, ShapedFile, text};
use crate::budget::DIFF_CHUNK_MAX_BYTES;

const OVERSIZED: &str = "diff exceeds one whole-file evidence chunk";
/// Bytes Git inspects for a NUL when it decides a blob is binary.
const BINARY_PROBE_BYTES: usize = 8000;

/// The review disposition of one committed path.
pub(super) enum FileDecision {
  Skipped(&'static str),
  Unreviewed(&'static str),
  Reviewed(ShapedFile),
}

/// Read a committed blob and its patch, preserving noise and budget reasons.
pub(super) fn inspect_file(
  context: &FileContext<'_>,
  status: &Status,
) -> Result<FileDecision, ShapeError> {
  let generated = match context.generated {
    Some(paths) => paths.contains(&status.path),
    // A failed batch is retried for this path alone, so one bad path cannot hide the rest.
    None => batch_generated(context.env, context.root, &[status], context.merge_base)
      .ok_or_else(|| ShapeError::Git("generated-file classification failed".to_owned()))?
      .contains(&status.path),
  };
  if generated {
    return Ok(FileDecision::Skipped("generated"));
  }
  let (patch, truncated) = file_patch(context, status)?;
  if truncated {
    return Ok(FileDecision::Unreviewed(OVERSIZED));
  }
  let patch = text(patch)?;
  // Mixed sections (a binary file replaced by a symlink) must not pass as a binary skip.
  if patch.contains("\ndiff --git ") {
    return Ok(FileDecision::Unreviewed(
      "Git patch has more than one file section",
    ));
  }
  // Git's own verdict sits on a header line; changed text always carries a prefix.
  if patch.lines().any(|line| line.starts_with("Binary files ")) {
    return binary(context, status);
  }
  if patch.len() > DIFF_CHUNK_MAX_BYTES {
    return Ok(FileDecision::Unreviewed(OVERSIZED));
  }
  let lines = patch_lines(&patch)?;
  if lines.is_empty() && status.kind != FileKind::Renamed {
    return Ok(FileDecision::Unreviewed(
      "no changed-line anchors in Git patch",
    ));
  }
  Ok(FileDecision::Reviewed(ShapedFile {
    path: status.path.clone(),
    old_path: status.old_path.clone(),
    kind: status.kind,
    patch,
    lines,
  }))
}

/// Git's binary verdict as a skip, unless only one committed side holds a NUL byte:
/// the branch must not hide a text change behind one.
fn binary(context: &FileContext<'_>, status: &Status) -> Result<FileDecision, ShapeError> {
  if matches!(status.kind, FileKind::Added | FileKind::Deleted) {
    return Ok(FileDecision::Skipped("binary"));
  }
  let base = has_nul(context, context.merge_base, old_side(status))?;
  if base != has_nul(context, "HEAD", &status.path)? {
    return Ok(FileDecision::Unreviewed(
      "content switched between text and binary",
    ));
  }
  Ok(FileDecision::Skipped("binary"))
}

/// Git's content test: a NUL byte within the blob's first 8000 bytes.
fn has_nul(context: &FileContext<'_>, rev: &str, path: &str) -> Result<bool, ShapeError> {
  let object = format!("{rev}:{path}");
  let args = ["cat-file", "blob", object.as_str()];
  let (head, _) = git_output(context.env, context.root, &args, BINARY_PROBE_BYTES)?;
  Ok(head.contains(&0))
}

/// The zero-context patch of exactly this entry's literal path or rename pair,
/// read only up to one chunk past the budget, and whether more was dropped.
/// Merge-base attributes keep the branch from turning its own source into a binary diff.
fn file_patch(context: &FileContext<'_>, status: &Status) -> Result<(Vec<u8>, bool), ShapeError> {
  let attr_source = format!("--attr-source={}", context.merge_base);
  let mut args = vec![
    attr_source.as_str(),
    "--literal-pathspecs",
    "diff",
    "--no-color",
    "--no-ext-diff",
    "--no-textconv",
    // Pin formats user config can change: diff.submodule=log drops the hunk, and
    // diff.interHunkContext adds context lines that diff.suppressBlankEmpty empties.
    "--submodule=short",
    "--inter-hunk-context=0",
    "-M",
    "--unified=0",
    context.range,
    "--",
  ];
  if let Some(old_path) = &status.old_path {
    args.push(old_path);
  }
  if status.old_path.as_deref() != Some(status.path.as_str()) {
    args.push(&status.path);
  }
  git_output(context.env, context.root, &args, DIFF_CHUNK_MAX_BYTES + 1)
}

#[cfg(test)]
#[path = "tests/file_test.rs"]
mod tests;
