//! A fixer's agent name (from the slot's POSIX `cksum`) and the files beside
//! the slot state that belong to a round's group.

use std::path::{Path, PathBuf};

use crate::state::sibling;

/// POSIX `cksum` of `bytes`: CRC-32/04C11DB7 over the data, then its length.
pub(crate) fn cksum(bytes: &[u8]) -> u32 {
  let mut crc: u32 = 0;
  let mut step = |byte: u8| {
    crc ^= u32::from(byte) << 24;
    for _ in 0..8 {
      crc = if crc & 0x8000_0000 == 0 {
        crc << 1
      } else {
        (crc << 1) ^ 0x04c1_1db7
      };
    }
  };
  for byte in bytes {
    step(*byte);
  }
  let mut length = bytes.len();
  while length > 0 {
    step(length.to_le_bytes().first().copied().unwrap_or_default());
    length >>= 8;
  }
  !crc
}

/// `pb-<cksum mod 2^24 as 6 hex>-r<round>g<seq>`.
pub(crate) fn agent_name(slot: &str, round: u64, seq: u64) -> String {
  format!(
    "pb-{:06x}-r{round}g{seq}",
    cksum(slot.as_bytes()) % 16_777_216
  )
}

/// `<state stem>.fixer-r<round>g<seq><suffix>`: `.md` or `.report.json`.
pub(crate) fn group_file(state: &Path, round: u64, seq: u64, suffix: &str) -> PathBuf {
  sibling(state, &format!(".fixer-r{round}g{seq}{suffix}"))
}

/// `text` in single quotes for a POSIX shell.
pub(crate) fn shell_quote(text: &str) -> String {
  format!("'{}'", text.replace('\'', "'\\''"))
}

#[cfg(test)]
#[path = "tests/names_test.rs"]
mod tests;
