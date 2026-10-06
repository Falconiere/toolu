//! A bound on tree-sitter-bash's serialized heredoc stack (#416): 4 bytes, then
//! 7 and the delimiter with its NUL per heredoc. Its bounds check is short by
//! four, so 1,025 to 1,027 bytes abort the process (or overrun the buffer).
//! The stack keeps stale entries and error recovery lexes a `<<` again, once
//! more for each other heredoc it recovers around, so the bound counts every
//! alignment of a run of `<`, times the number of heredocs (at least `RELEX`).

/// The state `parse` refuses: the 1024-byte buffer less the scanner's short check.
pub(crate) const SCANNER_STATE_LIMIT: usize = 1000;

/// A delimiter this long never fits the scanner's check (`size + 3 + 4 >= 1024`).
const HUGE: usize = 1017;

/// The fewest times one `<<` is assumed to reach the stack; more heredocs
/// raise it (docs/shell-analysis.md has the measurement).
const RELEX: usize = 2;

/// An upper bound of the bytes tree-sitter-bash's scanner can serialize for
/// `source`. Counting stops once it reaches `SCANNER_STATE_LIMIT`.
pub(crate) fn heredoc_state(source: &str) -> usize {
  if !source.contains("<<") {
    return 4;
  }
  let words = Words {
    source,
    short: runs(source, char::is_whitespace),
    long: runs(source, blank),
  };
  let bytes = source.as_bytes();
  let (mut size, mut pushes, mut at) = (0, 0, 0);
  // Words reach the stack only once a heredoc is on it: with no push, nothing is stored.
  while pushes == 0 || 4 + pushes.max(RELEX) * size < SCANNER_STATE_LIMIT {
    let Some(start) = bytes
      .get(at..)
      .and_then(|rest| rest.iter().position(|byte| *byte == b'<'))
      .map(|found| at + found)
    else {
      break;
    };
    let run = bytes.get(start..).unwrap_or_default();
    at = start + run.iter().take_while(|byte| **byte == b'<').count();
    let (bytes, push) = words.run(at - start, at);
    size += bytes;
    pushes += usize::from(push);
  }
  if pushes == 0 {
    return 4;
  }
  4 + pushes.max(RELEX) * size
}

/// The words of one source, read as `advance_word` reads them.
struct Words<'s> {
  source: &'s str,
  /// Unquoted lengths from each offset, ending at any whitespace (a lower bound).
  short: Vec<u16>,
  /// Unquoted lengths from each offset, ending at ASCII whitespace (an upper bound).
  long: Vec<u16>,
}

impl Words<'_> {
  /// What a run of `length` `<` ending at `end` can add: 7 for its push
  /// (none before `=`), the word after each `<<` in it, and the word after
  /// `<<-`; and whether it pushes.
  fn run(&self, length: usize, end: usize) -> (usize, bool) {
    if length < 2 {
      return (0, false);
    }
    let next = self.source.get(end..).and_then(|rest| rest.chars().next());
    let push = if next == Some('=') { 0 } else { 7 };
    let (short, long) = (at(&self.short, end), at(&self.long, end));
    let inner: usize = (1..length - 1)
      .map(|left| entry(left + short, left + long))
      .sum();
    let after = self.word(end);
    let dashed = if next == Some('-') {
      self.word(end + 1)
    } else {
      0
    };
    (push + inner + after + dashed, push > 0)
  }

  /// The delimiter a `<<` token before `offset` can append: whitespace is
  /// skipped, then an unquoted or quoted word. Non-ASCII whitespace the C
  /// library may keep counts too, and then no length is sure.
  fn word(&self, offset: usize) -> usize {
    let text = self.source.get(offset..).unwrap_or_default();
    let start = text.trim_start_matches(char::is_whitespace);
    let skipped = text.get(..text.len() - start.len()).unwrap_or_default();
    let unsure = skipped.chars().filter(|c| !blank(*c)).count();
    let from = offset + skipped.len();
    let quoted = match start.chars().next() {
      Some(quote @ ('\'' | '"')) => Some(quoted(start.get(1..).unwrap_or_default(), quote)),
      _ => None,
    };
    let (short, long) = (at(&self.short, from), at(&self.long, from));
    match quoted {
      _ if unsure > 0 => entry(0, unsure + long.max(quoted.unwrap_or(0))),
      Some(quoted) => entry(quoted, quoted),
      None => entry(short, long),
    }
  }
}

/// The length `lengths` holds for `offset`.
fn at(lengths: &[u16], offset: usize) -> usize {
  lengths.get(offset).copied().map_or(0, usize::from)
}

/// What a delimiter of `short` to `long` characters adds to the state: nothing
/// when it is sure to fail the scanner's check, else its length and NUL.
fn entry(short: usize, long: usize) -> usize {
  if short >= HUGE { 0 } else { long.min(HUGE) + 1 }
}

/// Whitespace in every C library's `iswspace`.
fn blank(c: char) -> bool {
  matches!(c, ' ' | '\t' | '\n' | '\x0b' | '\x0c' | '\r')
}

/// For each byte offset of `source`, the characters an unquoted `advance_word`
/// keeps from there before `stop` or NUL, capped at `HUGE`; a backslash keeps
/// the character after it.
fn runs(source: &str, stop: impl Fn(char) -> bool) -> Vec<u16> {
  let mut lengths = vec![0_u16; source.len() + 1];
  let (mut next, mut after, mut following) = (source.len(), source.len(), None);
  for (at, c) in source.char_indices().rev() {
    let kept = |offset: usize| 1 + lengths.get(offset).copied().map_or(0, usize::from);
    let length = match (c, following) {
      ('\0', _) => 0,
      _ if stop(c) => 0,
      ('\\', None | Some('\0')) => 0,
      ('\\', Some(_)) => kept(after),
      _ => kept(next),
    };
    if let Some(slot) = lengths.get_mut(at) {
      *slot = u16::try_from(length.min(HUGE)).unwrap_or(u16::MAX);
    }
    (after, next, following) = (next, at, Some(c));
  }
  lengths
}

/// The characters a quoted `advance_word` keeps from `inside` before `quote`,
/// CR, LF or NUL, capped at `HUGE`.
fn quoted(inside: &str, quote: char) -> usize {
  let mut chars = inside.chars();
  let mut kept = 0;
  while let Some(c) = chars.next() {
    if matches!(c, '\0' | '\r' | '\n') || c == quote || kept >= HUGE {
      break;
    }
    if c == '\\' && chars.next().is_none_or(|escaped| escaped == '\0') {
      break;
    }
    kept += 1;
  }
  kept
}

#[cfg(test)]
#[path = "tests/scanner_test.rs"]
mod tests;
