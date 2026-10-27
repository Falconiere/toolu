//! Keys from a terminal in raw mode. An escape byte alone is Esc only when no
//! sequence follows it soon; `\x1b[A` and `\x1b[B` (or `\x1bOA`, `\x1bOB`) are
//! the arrows, and any other sequence is ignored.

use std::io;

/// What a key press means to a picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Key {
  /// ↑ or `k`.
  Up,
  /// ↓ or `j`.
  Down,
  /// Space.
  Toggle,
  /// `a`: all on, or all off when all are on.
  All,
  /// Enter.
  Enter,
  /// `q`, Esc, Ctrl-C or the end of input.
  Cancel,
  /// Anything else.
  Other,
}

/// Where key bytes come from.
pub(crate) trait KeyInput {
  /// The next byte, waiting for it; `None` at the end of input.
  fn byte(&mut self) -> io::Result<Option<u8>>;
  /// The next byte if one arrives soon (the tail of an escape sequence).
  fn byte_soon(&mut self) -> io::Result<Option<u8>>;
}

/// The next key from `input`.
pub(crate) fn read_key(input: &mut impl KeyInput) -> io::Result<Key> {
  let Some(byte) = input.byte()? else {
    return Ok(Key::Cancel);
  };
  Ok(match byte {
    b'k' => Key::Up,
    b'j' => Key::Down,
    b' ' => Key::Toggle,
    b'a' => Key::All,
    b'\r' | b'\n' => Key::Enter,
    b'q' | 0x03 => Key::Cancel,
    0x1b => escape(input)?,
    _other => Key::Other,
  })
}

fn escape(input: &mut impl KeyInput) -> io::Result<Key> {
  match input.byte_soon()? {
    None => Ok(Key::Cancel),
    Some(b'[' | b'O') => Ok(match input.byte_soon()? {
      Some(b'A') => Key::Up,
      Some(b'B') => Key::Down,
      _unknown => Key::Other,
    }),
    Some(_unknown) => Ok(Key::Other),
  }
}

#[cfg(test)]
#[path = "tests/keys_test.rs"]
mod tests;
