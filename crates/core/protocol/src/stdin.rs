//! The hook payload: the host writes one JSON document on standard input.

use std::io::Read;

/// Read all of standard input as text.
///
/// # Errors
/// When standard input cannot be read or is not UTF-8.
pub fn read_stdin() -> std::io::Result<String> {
  read_all(std::io::stdin())
}

/// Read `reader` to the end as UTF-8 text.
///
/// # Errors
/// When `reader` fails or its bytes are not UTF-8.
pub fn read_all(mut reader: impl Read) -> std::io::Result<String> {
  let mut text = String::new();
  reader.read_to_string(&mut text)?;
  Ok(text)
}

#[cfg(test)]
#[path = "tests/stdin_test.rs"]
mod tests;
