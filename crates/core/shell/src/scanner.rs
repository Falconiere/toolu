//! A bound on tree-sitter-bash's serialized heredoc stack (#416): 4 bytes, then
//! 7 and the delimiter per heredoc. Its bounds check is short by four, so a
//! state near 1024 bytes aborts the process in tree-sitter's assertion.
//!
//! A heredoc is pushed at most once per run of `<` (at a `<<` followed by
//! neither `<` nor `=`). The word after a `<<` token, read to whitespace or
//! NUL (quoted: to the quote, CR, LF or NUL), is appended to its delimiter; a
//! lexer leaves that token where a run's length is 2 more than a multiple of 3.

/// The state `parse` refuses: the 1024-byte buffer less the scanner's short check.
pub(crate) const SCANNER_STATE_LIMIT: usize = 1000;

/// Heredocs the scanner may push for `source`, and an upper bound of the bytes
/// it serializes. Counting stops once the bound reaches `SCANNER_STATE_LIMIT`.
pub(crate) fn heredoc_state(source: &str) -> (usize, usize) {
  let bytes = source.as_bytes();
  let (mut count, mut size, mut at) = (0, 4, 0);
  while size < SCANNER_STATE_LIMIT {
    let Some(start) = bytes
      .get(at..)
      .and_then(|rest| rest.iter().position(|byte| *byte == b'<'))
      .map(|found| at + found)
    else {
      break;
    };
    let run = bytes.get(start..).unwrap_or_default();
    at = start + run.iter().take_while(|byte| **byte == b'<').count();
    // A first `<` escaped by a backslash belongs to a word.
    let before = bytes.get(..start).unwrap_or_default();
    let escaped = before
      .iter()
      .rev()
      .take_while(|byte| **byte == b'\\')
      .count()
      % 2;
    let length = at - start - escaped;
    if length < 2 {
      continue;
    }
    if bytes.get(at) != Some(&b'=') {
      count += 1;
      size += 7;
    }
    if length % 3 == 2 {
      let after = source.get(at..).unwrap_or_default();
      size += after
        .strip_prefix('-')
        .map_or_else(|| word(after), |dashed| 1 + word(dashed));
    }
  }
  (count, size)
}

/// Whitespace in every C library's `iswspace`.
fn blank(c: char) -> bool {
  matches!(c, ' ' | '\t' | '\n' | '\x0b' | '\x0c' | '\r')
}

/// The most `advance_word` can append to a delimiter from `text`: its
/// characters and a NUL. Non-ASCII whitespace before it counts too, in case
/// the C library keeps it.
fn word(text: &str) -> usize {
  let start = text.trim_start_matches(char::is_whitespace);
  let skipped = text.get(..text.len() - start.len()).unwrap_or_default();
  let unsure = skipped.chars().filter(|c| !blank(*c)).count();
  let unquoted = kept(start, blank);
  let quoted = match start.chars().next() {
    Some(quote @ ('\'' | '"')) => {
      let inside = start.get(1..).unwrap_or_default();
      kept(inside, |c| c == quote || c == '\r' || c == '\n')
    }
    _ => 0,
  };
  1 + unsure + unquoted.max(quoted)
}

/// The characters `advance_word` keeps from `text` before `stop`, NUL or the
/// end, at most `SCANNER_STATE_LIMIT`.
fn kept(text: &str, stop: impl Fn(char) -> bool) -> usize {
  let mut chars = text.chars();
  let mut kept = 0;
  while let Some(c) = chars.next() {
    if c == '\0' || stop(c) || kept >= SCANNER_STATE_LIMIT {
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
