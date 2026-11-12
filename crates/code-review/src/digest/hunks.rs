//! Bounded hunks of a zero-context patch: at most three per file and eight
//! changed lines each, within one hunk-text budget for the whole digest.

use serde::Serialize;

/// The most hunks printed per file.
const MAX_HUNKS: usize = 3;
/// The most changed lines printed per hunk.
const HUNK_LINES: usize = 8;
/// The most hunk text, in bytes, one digest prints.
const HUNK_BYTES: usize = 6 * 1024;

/// One hunk: its header, its first changed lines, and what is left to read.
#[derive(Debug, Serialize)]
pub(super) struct Hunk {
  pub(super) header: String,
  pub(super) lines: Vec<String>,
  /// `... <n> more lines: read new a-b` when the hunk was cut.
  pub(super) more: Option<String>,
}

/// The hunk-text budget shared by every file of one digest. Once a file's
/// hunks would pass it, that file and every later one print no hunks.
#[derive(Default)]
pub(super) struct Budget {
  used: usize,
  /// How many files printed no hunks because the budget ran out.
  pub(super) dropped: usize,
}

impl Budget {
  /// The hunks of `patch` to print, and how many of its hunks are left out.
  pub(super) fn take(&mut self, patch: &str) -> (Vec<Hunk>, usize) {
    let all = split(patch);
    let total = all.len();
    let shown: Vec<Hunk> = all.into_iter().take(MAX_HUNKS).map(cut).collect();
    let bytes: usize = shown.iter().map(size).sum();
    if self.dropped > 0 || self.used + bytes > HUNK_BYTES {
      if total > 0 {
        self.dropped += 1;
      }
      return (Vec::new(), total);
    }
    self.used += bytes;
    let left = total - shown.len();
    (shown, left)
  }
}

fn size(hunk: &Hunk) -> usize {
  hunk.header.len() + hunk.lines.iter().map(|line| line.len() + 1).sum::<usize>()
}

/// Each `@@` header with its changed lines, in patch order.
fn split(patch: &str) -> Vec<(String, Vec<String>)> {
  let mut hunks: Vec<(String, Vec<String>)> = Vec::new();
  for line in patch.split_terminator('\n') {
    if line.starts_with("@@ ") {
      hunks.push((line.to_owned(), Vec::new()));
    } else if let Some((_, lines)) = hunks.last_mut()
      && (line.starts_with('+') || line.starts_with('-'))
    {
      lines.push(line.to_owned());
    }
  }
  hunks
}

/// The first `HUNK_LINES` lines of a hunk and the old and new ranges left.
fn cut((header, mut lines): (String, Vec<String>)) -> Hunk {
  let left = lines.len().saturating_sub(HUNK_LINES);
  lines.truncate(HUNK_LINES);
  let more = (left > 0).then(|| {
    let ranges = remaining(&header).unwrap_or_default();
    if ranges.is_empty() {
      format!("... {left} more lines")
    } else {
      format!("... {left} more lines: read {ranges}")
    }
  });
  Hunk {
    header,
    lines,
    more,
  }
}

/// The unshown ranges of a zero-context hunk, whose removals precede its additions.
fn remaining(header: &str) -> Option<String> {
  let mut parts = header.split_whitespace().skip(1);
  let (old_start, old_count) = range(parts.next()?.strip_prefix('-')?)?;
  let (new_start, new_count) = range(parts.next()?.strip_prefix('+')?)?;
  let shown_old = old_count.min(HUNK_LINES);
  let shown_new = (HUNK_LINES - shown_old).min(new_count);
  let mut out = Vec::new();
  if old_count > shown_old {
    out.push(format!(
      "old {}-{}",
      old_start + shown_old,
      old_start + old_count - 1
    ));
  }
  if new_count > shown_new {
    out.push(format!(
      "new {}-{}",
      new_start + shown_new,
      new_start + new_count - 1
    ));
  }
  Some(out.join(", "))
}

/// `start,count` of one hunk side; a bare `start` counts one line.
fn range(text: &str) -> Option<(usize, usize)> {
  let (start, count) = text.split_once(',').unwrap_or((text, "1"));
  Some((start.parse().ok()?, count.parse().ok()?))
}

#[cfg(test)]
#[path = "tests/hunks_test.rs"]
mod tests;
