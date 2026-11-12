//! The digest as text: a header line, one block per file, then skipped and unread paths.

use std::fmt::Write as _;

use super::hunks::Hunk;
use super::{Digest, FileDigest};
use crate::shape::FileKind;

/// The digest text, or `No changes against <base>` when nothing changed.
pub(super) fn text(digest: &Digest) -> String {
  if digest.files.is_empty() && digest.skipped.is_empty() && digest.unread.is_empty() {
    return format!("No changes against {}", digest.base);
  }
  let mut out = header(digest);
  if let Some(reason) = &digest.items_unavailable {
    let _ = write!(out, "\nnote: items unavailable: {reason}");
  }
  for file in &digest.files {
    out.push('\n');
    out.push_str(&file_text(file, digest.items_unavailable.is_some()));
  }
  if digest.files_omitted > 0 {
    let _ = write!(
      out,
      "\n... {} more files; pass --path to narrow",
      digest.files_omitted
    );
  }
  if digest.hunks_budget_files > 0 {
    let _ = write!(
      out,
      "\nhunks omitted for {} files past 6 KB; pass --path to see them",
      digest.hunks_budget_files
    );
  }
  for skipped in &digest.skipped {
    let _ = write!(out, "\nskipped: {} ({})", skipped.path, skipped.reason);
  }
  for unread in &digest.unread {
    let _ = write!(out, "\nunread: {} ({})", unread.path, unread.reason);
  }
  out
}

fn header(digest: &Digest) -> String {
  let short = digest.merge_base.get(..7).unwrap_or(&digest.merge_base);
  let local = if digest.uncommitted {
    " + uncommitted"
  } else {
    ""
  };
  format!(
    "digest: {} (merge-base {short}){local}, {} files, +{} -{}",
    digest.base,
    digest.files.len() + digest.files_omitted,
    digest.added,
    digest.removed
  )
}

fn file_text(file: &FileDigest, unavailable: bool) -> String {
  let from = file
    .old_path
    .as_ref()
    .map(|old| format!(" (from {old})"))
    .unwrap_or_default();
  let kind = match file.kind {
    FileKind::Added => "added",
    FileKind::Modified => "modified",
    FileKind::Deleted => "deleted",
    FileKind::Renamed => "renamed",
    FileKind::Other => "other",
  };
  let mut out = format!(
    "{}{from} +{} -{} ({kind})",
    file.path, file.added, file.removed
  );
  if unavailable {
    out.push_str("\n  items unavailable");
  } else if !file.items.is_empty() {
    let _ = write!(out, "\n  items: {}", items(file));
  }
  if !file.removed_items.is_empty() {
    let _ = write!(out, "\n  removed: {}", file.removed_items.join(", "));
  }
  for hunk in &file.hunks {
    hunk_text(&mut out, hunk);
  }
  if !file.hunks.is_empty() && file.hunks_omitted > 0 {
    let _ = write!(out, "\n  ... {} more hunks", file.hunks_omitted);
  }
  out
}

/// `name start-end` per item, `(new)` when wholly added, then `+<n> more`.
fn items(file: &FileDigest) -> String {
  let names: Vec<String> = file
    .items
    .iter()
    .map(|item| {
      let new = if item.new { " (new)" } else { "" };
      format!("{} {}-{}{new}", item.name, item.start, item.end)
    })
    .collect();
  let mut text = names.join(", ");
  if file.items_omitted > 0 {
    let _ = write!(text, ", +{} more", file.items_omitted);
  }
  text
}

fn hunk_text(out: &mut String, hunk: &Hunk) {
  let _ = write!(out, "\n  {}", hunk.header);
  for line in &hunk.lines {
    let _ = write!(out, "\n  {line}");
  }
  if let Some(more) = &hunk.more {
    let _ = write!(out, "\n  {more}");
  }
}

#[cfg(test)]
#[path = "tests/render_test.rs"]
mod tests;
