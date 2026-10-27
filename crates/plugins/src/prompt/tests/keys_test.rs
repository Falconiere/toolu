use std::collections::VecDeque;
use std::io;

use super::{Key, KeyInput, read_key};

/// Bytes already typed: nothing more ever arrives.
struct Typed(VecDeque<u8>);

impl KeyInput for Typed {
  fn byte(&mut self) -> io::Result<Option<u8>> {
    Ok(self.0.pop_front())
  }

  fn byte_soon(&mut self) -> io::Result<Option<u8>> {
    Ok(self.0.pop_front())
  }
}

fn keys(bytes: &[u8]) -> Vec<Key> {
  let mut typed = Typed(bytes.iter().copied().collect());
  let mut keys = Vec::new();
  while !typed.0.is_empty() {
    keys.push(read_key(&mut typed).expect("a key"));
  }
  keys
}

#[test]
fn arrows_letters_space_enter_and_the_cancel_keys_map() {
  assert_eq!(
    keys(b"\x1b[A\x1b[Bkj a\r\nq\x03\x1bOA"),
    [
      Key::Up,
      Key::Down,
      Key::Up,
      Key::Down,
      Key::Toggle,
      Key::All,
      Key::Enter,
      Key::Enter,
      Key::Cancel,
      Key::Cancel,
      Key::Up
    ]
  );
}

#[test]
fn a_lone_escape_cancels_and_an_unknown_sequence_is_ignored() {
  assert_eq!(keys(b"\x1b"), [Key::Cancel]);
  assert_eq!(keys(b"\x1b[Z"), [Key::Other]);
  assert_eq!(keys(b"x"), [Key::Other]);
  let mut ended = Typed(VecDeque::new());
  assert_eq!(read_key(&mut ended).expect("a key"), Key::Cancel);
}
