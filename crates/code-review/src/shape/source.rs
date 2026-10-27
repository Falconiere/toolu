//! Batched committed attribute and generator-header classification.

use std::collections::BTreeSet;
use std::path::Path;

use toolu_runtime::env::Env;

use super::git_io::{GIT_OUTPUT_MAX_BYTES, git_output, git_process, git_process_input};
use super::noise::{HEADER_BYTES, HEADER_LINES, header_generated};
use super::parse::Status;
use super::{FileKind, ShapeError};

const BATCH_PATHS: usize = 500;

/// Collect generated paths with a bounded number of Git processes; `None` when a
/// batch fails, so the caller can retry path by path. Attributes come from the
/// merge-base, and a changed or renamed file counts only when both its base and
/// HEAD sides are generated, so the reviewed branch cannot mark its own changes generated.
pub(super) fn batch_generated(
  env: &Env,
  root: &Path,
  statuses: &[&Status],
  merge_base: &str,
) -> Option<BTreeSet<String>> {
  let new_paths: Vec<&str> = statuses
    .iter()
    .filter(|status| status.kind != FileKind::Deleted)
    .map(|status| status.path.as_str())
    .collect();
  let old_paths: Vec<&str> = statuses
    .iter()
    .filter(|status| status.kind != FileKind::Added)
    .map(|status| old_side(status))
    .collect();
  // Both sides read merge-base attributes, so each distinct path is asked once.
  let all_paths: BTreeSet<&str> = new_paths.iter().chain(&old_paths).copied().collect();
  let all_paths: Vec<&str> = all_paths.into_iter().collect();
  let attributes = batched(&all_paths, |batch| {
    attribute_paths(env, root, merge_base, batch)
  })?;
  let new = batched(&new_paths, |batch| marker_batch(env, root, "HEAD", batch))?;
  let old = batched(&old_paths, |batch| {
    marker_batch(env, root, merge_base, batch)
  })?;
  let generated = statuses
    .iter()
    .filter(|status| {
      let old_path = old_side(status);
      let new = attributes.contains(&status.path) || new.contains(&status.path);
      let old = attributes.contains(old_path) || old.contains(old_path);
      match status.kind {
        FileKind::Added => new,
        FileKind::Deleted => old,
        FileKind::Modified | FileKind::Renamed | FileKind::Other => new && old,
      }
    })
    .map(|status| status.path.clone())
    .collect();
  Some(generated)
}

/// The path a status had at the merge-base.
pub(super) fn old_side(status: &Status) -> &str {
  status.old_path.as_deref().unwrap_or(&status.path)
}

/// One check's matches over bounded path batches; `None` when any batch fails.
fn batched(
  paths: &[&str],
  mut check: impl FnMut(&[&str]) -> Result<BTreeSet<String>, ShapeError>,
) -> Option<BTreeSet<String>> {
  let mut found = BTreeSet::new();
  for batch in paths.chunks(BATCH_PATHS) {
    found.extend(check(batch).ok()?);
  }
  Some(found)
}

/// Paths whose `linguist-generated` attribute is set at `source`.
fn attribute_paths(
  env: &Env,
  root: &Path,
  source: &str,
  paths: &[&str],
) -> Result<BTreeSet<String>, ShapeError> {
  let mut input = Vec::new();
  for path in paths {
    input.extend_from_slice(path.as_bytes());
    input.push(0);
  }
  let source_arg = format!("--source={source}");
  let args = [
    "check-attr",
    &source_arg,
    "-z",
    "--stdin",
    "linguist-generated",
  ];
  let out = git_process_input(env, root, &args, GIT_OUTPUT_MAX_BYTES, input)?;
  if out.exit_code != 0 || out.truncated || out.timed_out {
    return Err(ShapeError::Git("batched git check-attr failed".to_owned()));
  }
  parse_attributes(&out.stdout_bytes, paths)
}

fn parse_attributes(bytes: &[u8], paths: &[&str]) -> Result<BTreeSet<String>, ShapeError> {
  let bytes = bytes
    .strip_suffix(&[0])
    .ok_or_else(|| ShapeError::Git("unterminated batched git check-attr output".to_owned()))?;
  let fields: Vec<&[u8]> = bytes.split(|byte| *byte == 0).collect();
  let expected: BTreeSet<&str> = paths.iter().copied().collect();
  if fields.len() != paths.len() * 3 {
    return Err(ShapeError::Git(
      "incomplete batched git attributes".to_owned(),
    ));
  }
  let mut seen = BTreeSet::new();
  let mut generated = BTreeSet::new();
  // The length check above leaves no remainder.
  let (rows, _) = fields.as_chunks::<3>();
  for &[raw_path, attribute, value] in rows {
    let path = std::str::from_utf8(raw_path)
      .map_err(|err| ShapeError::Git(format!("non-UTF-8 attribute path: {err}")))?;
    if !expected.contains(path) || !seen.insert(path) || attribute != b"linguist-generated" {
      return Err(ShapeError::Git(
        "unexpected batched git attribute".to_owned(),
      ));
    }
    if value == b"set" || value == b"true" {
      generated.insert(path.to_owned());
    }
  }
  Ok(generated)
}

fn marker_paths(
  env: &Env,
  root: &Path,
  source: &str,
  paths: &[&str],
) -> Result<Option<BTreeSet<String>>, ShapeError> {
  let max_count = HEADER_LINES.to_string();
  let mut args = vec![
    "--literal-pathspecs",
    "grep",
    "--no-color",
    // `grep.column` would add a field the parser reads as line text.
    "--no-column",
    "-n",
    "-z",
    "-I",
    "-i",
    "-m",
    &max_count,
    "-e",
    "generated",
    source,
    "--",
  ];
  args.extend(paths.iter().copied());
  let out = git_process(env, root, &args, GIT_OUTPUT_MAX_BYTES)?;
  if out.timed_out || !matches!(out.exit_code, 0 | 1) {
    return Err(ShapeError::Git("batched git grep failed".to_owned()));
  }
  if out.truncated {
    return Ok(None);
  }
  let expected: BTreeSet<&str> = paths.iter().copied().collect();
  parse_matches(&out.stdout_bytes, source, &expected).map(Some)
}

fn marker_prefix(
  env: &Env,
  root: &Path,
  source: &str,
  path: &str,
) -> Result<BTreeSet<String>, ShapeError> {
  let object = format!("{source}:{path}");
  let (header, _) = git_output(env, root, &["show", &object], HEADER_BYTES)?;
  Ok(if header_generated(&header) {
    BTreeSet::from([path.to_owned()])
  } else {
    BTreeSet::new()
  })
}

fn marker_batch(
  env: &Env,
  root: &Path,
  source: &str,
  paths: &[&str],
) -> Result<BTreeSet<String>, ShapeError> {
  match marker_paths(env, root, source, paths)? {
    Some(found) => Ok(found),
    None if paths.len() == 1 => {
      let path = paths
        .first()
        .copied()
        .ok_or_else(|| ShapeError::Git("empty marker batch".to_owned()))?;
      marker_prefix(env, root, source, path)
    }
    None => {
      let (left, right) = paths.split_at(paths.len() / 2);
      let mut found = marker_batch(env, root, source, left)?;
      found.extend(marker_batch(env, root, source, right)?);
      Ok(found)
    }
  }
}

fn parse_matches(
  mut bytes: &[u8],
  source: &str,
  expected: &BTreeSet<&str>,
) -> Result<BTreeSet<String>, ShapeError> {
  let prefix = format!("{source}:");
  let mut generated = BTreeSet::new();
  while !bytes.is_empty() {
    let (file, rest) = take_until(bytes, 0)?;
    let (line, rest) = take_until(rest, 0)?;
    let (content, rest) = take_until(rest, b'\n')?;
    let path = file
      .strip_prefix(prefix.as_bytes())
      .and_then(|path| std::str::from_utf8(path).ok())
      .ok_or_else(|| ShapeError::Git("invalid batched git grep path".to_owned()))?;
    let number = std::str::from_utf8(line)
      .ok()
      .and_then(|line| line.parse::<usize>().ok())
      .ok_or_else(|| ShapeError::Git("invalid batched git grep line".to_owned()))?;
    if !expected.contains(path) {
      return Err(ShapeError::Git(
        "unexpected batched git grep path".to_owned(),
      ));
    }
    if number <= HEADER_LINES && header_generated(content) {
      generated.insert(path.to_owned());
    }
    bytes = rest;
  }
  Ok(generated)
}

fn take_until(bytes: &[u8], delimiter: u8) -> Result<(&[u8], &[u8]), ShapeError> {
  let end = bytes
    .iter()
    .position(|byte| *byte == delimiter)
    .ok_or_else(|| ShapeError::Git("unterminated batched git grep result".to_owned()))?;
  let before = bytes
    .get(..end)
    .ok_or_else(|| ShapeError::Git("invalid batched git grep delimiter".to_owned()))?;
  let after = bytes
    .get(end + 1..)
    .ok_or_else(|| ShapeError::Git("invalid batched git grep delimiter".to_owned()))?;
  Ok((before, after))
}

#[cfg(test)]
#[path = "tests/source_test.rs"]
mod tests;
