//! Code-line counting (`detect-lines.ts`): `count_code_lines`, `count_python_code_lines`
//! and `has_unterminated_block`, reading the file in bounded chunks instead of
//! shelling out to awk, grep and wc. Lines are bytes, so a non-UTF-8 file counts as
//! awk counts it, and only spaces and tabs are blanks. `None` means the file could not
//! be read, where awk printed nothing.

use std::borrow::Cow;
use std::path::Path;

use super::is_regular_file;
use super::read::{Walk, each_line};

/// A line with its `/* */` blocks removed.
struct Stripped<'a> {
  /// What is left of the line.
  code: Cow<'a, [u8]>,
  /// Whether a block is still open at the end of the line.
  in_block: bool,
}

/// The first `needle` in `hay` at or after `from`.
fn find_from(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
  let at = hay
    .get(from..)?
    .windows(needle.len())
    .position(|window| window == needle)?;
  Some(from + at)
}

/// Strip `/* … */` blocks from `line`, given whether a block was already open; `None`
/// when the whole line is inside a block that stays open. A block removed can join its
/// neighbours into a new `/*`, so the search resumes one byte before the removal.
fn strip_blocks(line: &[u8], in_block: bool) -> Option<Stripped<'_>> {
  let mut rest = Cow::Borrowed(line);
  if in_block {
    let close = find_from(line, b"*/", 0)?;
    rest = Cow::Borrowed(line.get(close + 2..)?);
  }
  let mut from = 0;
  while let Some(open) = find_from(&rest, b"/*", from) {
    let Some(close) = find_from(&rest, b"*/", open + 2) else {
      rest.to_mut().truncate(open);
      return Some(Stripped {
        code: rest,
        in_block: true,
      });
    };
    rest.to_mut().drain(open..close + 2);
    from = open.saturating_sub(1);
  }
  Some(Stripped {
    code: rest,
    in_block: false,
  })
}

/// awk's blank: a space or a tab, so a `\r` is content.
fn is_blank(byte: u8) -> bool {
  matches!(byte, b' ' | b'\t')
}

/// Whether anything but blanks is left before a `//` comment.
fn has_code(code: &[u8]) -> bool {
  let end = find_from(code, b"//", 0).unwrap_or(code.len());
  let kept = code.get(..end).unwrap_or_default();
  kept.iter().copied().any(|byte| !is_blank(byte))
}

/// `count_code_lines FILE`: lines of code once blank lines, `//` comments and
/// `/* */` blocks are removed (TypeScript and Rust). String literals are not tracked,
/// so a `"/*"` can open a block that never closes; the count then falls back to the
/// raw line count, failing toward flagging a large file.
pub fn count_code_lines(path: &Path) -> Option<u64> {
  let (mut in_block, mut records, mut code) = (false, 0_u64, 0_u64);
  let walk = each_line(path, |line| {
    records += 1;
    if let Some(stripped) = strip_blocks(line, in_block) {
      in_block = stripped.in_block;
      code += u64::from(has_code(&stripped.code));
    }
    false
  });
  if walk == Walk::Unreadable {
    return None;
  }
  Some(if in_block { records } else { code })
}

/// `count_python_code_lines FILE`: non-blank lines that do not start with `#`.
/// Trailing comments and docstring lines count, failing toward flagging.
pub fn count_python_code_lines(path: &Path) -> Option<u64> {
  let mut code = 0_u64;
  let walk = each_line(path, |line| {
    let first = line.iter().copied().find(|byte| !is_blank(*byte));
    code += u64::from(first.is_some_and(|byte| byte != b'#'));
    false
  });
  (walk != Walk::Unreadable).then_some(code)
}

/// How many times `token` occurs in `line`, without overlap.
fn occurrences(line: &[u8], token: &[u8]) -> i64 {
  let (mut count, mut from) = (0, 0);
  while let Some(at) = find_from(line, token, from) {
    count += 1;
    from = at + token.len();
  }
  count
}

/// `has_unterminated_block FILE`: more `/*` than `*/` in a regular file. A file that
/// cannot be read has none, as the bash function's empty grep output reads.
pub fn has_unterminated_block(path: &Path) -> bool {
  if !is_regular_file(path) {
    return false;
  }
  let mut balance = 0_i64;
  // The walk's end does not matter: whatever was read before a failure still counts.
  each_line(path, |line| {
    balance += occurrences(line, b"/*") - occurrences(line, b"*/");
    false
  });
  balance > 0
}

#[cfg(test)]
#[path = "tests/lines_test.rs"]
mod tests;
