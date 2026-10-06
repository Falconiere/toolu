//! Bounded line reading for the detect layer (`detect-read.ts`): a file is read in
//! 64 KiB chunks, so memory stays flat whatever its size but for its longest line
//! (TypeScript also holds one line at a time), and every line is bytes, so
//! the awk and grep byte semantics the bash functions had hold for any encoding.

use std::fs::File;
use std::io::{BufRead as _, BufReader, ErrorKind};
use std::path::Path;

/// The bytes read from the file at a time.
const CHUNK: usize = 64 * 1024;

/// How a walk ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Walk {
  /// Every line was seen.
  Done,
  /// The visitor asked to stop.
  Stopped,
  /// The file could not be opened or read.
  Unreadable,
}

/// Call `visit` with each line of `path`, without its `\n`, as awk splits records: a
/// final unterminated line counts and a trailing newline starts no empty record.
/// `visit` returns `true` to stop early. A file that cannot be opened or read is
/// [`Walk::Unreadable`] (awk printed nothing then); a directory reads as empty, as awk
/// reads it.
pub(super) fn each_line<F: FnMut(&[u8]) -> bool>(path: &Path, mut visit: F) -> Walk {
  let Ok(file) = File::open(path) else {
    return Walk::Unreadable;
  };
  let mut reader = BufReader::with_capacity(CHUNK, file);
  let mut line = Vec::new();
  loop {
    line.clear();
    match reader.read_until(b'\n', &mut line) {
      Ok(0) => return Walk::Done,
      Ok(_) => {
        if line.last() == Some(&b'\n') {
          line.pop();
        }
        if visit(&line) {
          return Walk::Stopped;
        }
      }
      Err(err) if err.kind() == ErrorKind::IsADirectory => return Walk::Done,
      Err(_) => return Walk::Unreadable,
    }
  }
}

#[cfg(test)]
#[path = "tests/read_test.rs"]
mod tests;
