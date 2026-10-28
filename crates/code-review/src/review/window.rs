//! Line windows of a `HEAD` source too big to send whole: the lines around each
//! change, merged where they overlap. Each window is a `@@ lines a-b @@` header
//! and then its lines verbatim, so a quote taken from it still matches the
//! evidence a recheck checks it against.

use std::collections::BTreeSet;

use crate::budget::SOURCE_WINDOW_LINES;
use crate::contract::Side;
use crate::shape::{ShapedFile, hunk_start};

/// The windows that fit `cap` bytes, in file order and never cut, and how many
/// windows `file`'s changes have in `text` in all.
pub(super) fn windows(text: &str, file: &ShapedFile, cap: usize) -> (String, usize, usize) {
  let lines: Vec<&str> = text.split_inclusive('\n').collect();
  let ranges = ranges(&centers(file), lines.len());
  let mut out = String::new();
  let mut sent = 0;
  for (first, last) in &ranges {
    let mut body: String = lines
      .iter()
      .skip(first - 1)
      .take(last - first + 1)
      .copied()
      .collect();
    if !body.ends_with('\n') {
      body.push('\n');
    }
    let window = format!("@@ lines {first}-{last} @@\n{body}");
    if out.len() + window.len() > cap {
      break;
    }
    out.push_str(&window);
    sent += 1;
  }
  (out, sent, ranges.len())
}

/// New-side lines each change sits at: every added line, and each hunk's new
/// start, which is where a pure deletion leaves its mark.
fn centers(file: &ShapedFile) -> BTreeSet<usize> {
  let added = file
    .lines
    .iter()
    .filter(|line| line.side == Side::New)
    .map(|line| line.line);
  let hunks = file
    .patch
    .lines()
    .filter(|line| line.starts_with("@@ "))
    .filter_map(|line| hunk_start(line).ok())
    .map(|(_, new)| new);
  added
    .chain(hunks)
    .filter_map(|line| usize::try_from(line).ok())
    .map(|line| line.max(1))
    .collect()
}

/// One-based inclusive line ranges `SOURCE_WINDOW_LINES` around each center,
/// clamped to `total` lines and merged where they touch.
fn ranges(centers: &BTreeSet<usize>, total: usize) -> Vec<(usize, usize)> {
  let mut ranges: Vec<(usize, usize)> = Vec::new();
  for center in centers.iter().filter(|center| **center <= total) {
    let first = center.saturating_sub(SOURCE_WINDOW_LINES).max(1);
    let last = center.saturating_add(SOURCE_WINDOW_LINES).min(total);
    match ranges.last_mut() {
      Some(previous) if first <= previous.1 + 1 => previous.1 = previous.1.max(last),
      _ => ranges.push((first, last)),
    }
  }
  ranges
}

#[cfg(test)]
#[path = "tests/window_test.rs"]
mod tests;
