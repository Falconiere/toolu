//! NUL-delimited Git metadata and zero-context hunk parsing.

use crate::contract::Side;

use super::{ChangedLine, FileKind, ShapeError};

/// One status from Git's NUL-delimited name-status output.
pub(super) struct Status {
  pub kind: FileKind,
  pub old_path: Option<String>,
  pub path: String,
}

/// The exact gate-visible path set.
pub(super) fn names(bytes: &[u8]) -> Result<Vec<String>, ShapeError> {
  let mut names = fields(bytes)?;
  names.sort();
  names.dedup();
  Ok(names)
}

/// Decode Git's rename-aware status pairs.
pub(super) fn statuses(bytes: &[u8]) -> Result<Vec<Status>, ShapeError> {
  let fields = fields(bytes)?;
  let mut fields = fields.into_iter();
  let mut out = Vec::new();
  while let Some(code) = fields.next() {
    let kind = match code.as_bytes().first() {
      Some(b'A') => FileKind::Added,
      Some(b'M') => FileKind::Modified,
      Some(b'D') => FileKind::Deleted,
      Some(b'R') => FileKind::Renamed,
      Some(_) => FileKind::Other,
      None => return Err(ShapeError::Git("empty Git status".to_owned())),
    };
    let first = fields
      .next()
      .ok_or_else(|| ShapeError::Git("status missing path".to_owned()))?;
    let (old_path, path) = if kind == FileKind::Renamed {
      let new = fields
        .next()
        .ok_or_else(|| ShapeError::Git("rename missing new path".to_owned()))?;
      (Some(first), new)
    } else if kind == FileKind::Deleted {
      (Some(first.clone()), first)
    } else {
      (None, first)
    };
    out.push(Status {
      kind,
      old_path,
      path,
    });
  }
  Ok(out)
}

fn fields(bytes: &[u8]) -> Result<Vec<String>, ShapeError> {
  if bytes.is_empty() {
    return Ok(Vec::new());
  }
  let body = bytes
    .strip_suffix(&[0])
    .ok_or_else(|| ShapeError::Git("unterminated Git metadata".to_owned()))?;
  body
    .split(|byte| *byte == 0)
    .map(|field| {
      String::from_utf8(field.to_vec())
        .map_err(|err| ShapeError::Git(format!("non-UTF-8 path: {err}")))
    })
    .collect()
}

/// Extract only added and removed lines with their actual side and line.
pub(super) fn patch_lines(patch: &str) -> Result<Vec<ChangedLine>, ShapeError> {
  let mut lines = Vec::new();
  let mut old = 0u32;
  let mut new = 0u32;
  let mut in_hunk = false;
  for line in patch.split_terminator('\n') {
    if line.starts_with("@@ ") {
      (old, new) = hunk_start(line)?;
      in_hunk = true;
    } else if in_hunk {
      if let Some(text) = line.strip_prefix('+') {
        lines.push(ChangedLine {
          side: Side::New,
          line: new,
          text: text.to_owned(),
        });
        new = new
          .checked_add(1)
          .ok_or_else(|| ShapeError::Git("new line overflow".to_owned()))?;
      } else if let Some(text) = line.strip_prefix('-') {
        lines.push(ChangedLine {
          side: Side::Old,
          line: old,
          text: text.to_owned(),
        });
        old = old
          .checked_add(1)
          .ok_or_else(|| ShapeError::Git("old line overflow".to_owned()))?;
      } else if line.starts_with(' ') {
        old = old
          .checked_add(1)
          .ok_or_else(|| ShapeError::Git("old line overflow".to_owned()))?;
        new = new
          .checked_add(1)
          .ok_or_else(|| ShapeError::Git("new line overflow".to_owned()))?;
      } else if !line.starts_with("\\ No newline") {
        return Err(ShapeError::Git(format!("unexpected hunk line: {line}")));
      }
    }
  }
  Ok(lines)
}

/// The old and new start lines of a `@@ -a,b +c,d @@` hunk header.
pub(crate) fn hunk_start(line: &str) -> Result<(u32, u32), ShapeError> {
  let mut parts = line.split_whitespace();
  if parts.next() != Some("@@") {
    return Err(ShapeError::Git("bad hunk header".to_owned()));
  }
  let old = parts
    .next()
    .and_then(|part| part.strip_prefix('-'))
    .and_then(|part| part.split(',').next())
    .and_then(|part| part.parse().ok());
  let new = parts
    .next()
    .and_then(|part| part.strip_prefix('+'))
    .and_then(|part| part.split(',').next())
    .and_then(|part| part.parse().ok());
  old
    .zip(new)
    .ok_or_else(|| ShapeError::Git(format!("bad hunk header: {line}")))
}

#[cfg(test)]
#[path = "tests/parse_test.rs"]
mod tests;
