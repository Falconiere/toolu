//! SHA-1 (FIPS 180-4) as lowercase hex. Only finding keys and work
//! fingerprints use it, which name findings rather than protect anything, so
//! one small function replaces a dependency.

const INIT: [u32; 5] = [
  0x6745_2301,
  0xEFCD_AB89,
  0x98BA_DCFE,
  0x1032_5476,
  0xC3D2_E1F0,
];

/// The SHA-1 digest of `data` as 40 lowercase hex characters.
pub(crate) fn hex(data: &[u8]) -> String {
  let mut state = INIT;
  for block in padded(data).as_chunks::<64>().0 {
    compress(&mut state, block);
  }
  state
    .iter()
    .flat_map(|word| word.to_be_bytes())
    .flat_map(|byte| [byte >> 4, byte & 0xF])
    .map(|nibble| char::from_digit(u32::from(nibble), 16).unwrap_or('0'))
    .collect()
}

/// `data`, a `0x80` byte, zeros to 56 mod 64, and the bit length big-endian.
fn padded(data: &[u8]) -> Vec<u8> {
  let bits = u64::try_from(data.len())
    .unwrap_or(u64::MAX)
    .wrapping_mul(8);
  let mut message = data.to_vec();
  message.push(0x80);
  while message.len() % 64 != 56 {
    message.push(0);
  }
  message.extend_from_slice(&bits.to_be_bytes());
  message
}

fn word(words: &[u32], at: usize) -> u32 {
  words.get(at).copied().unwrap_or(0)
}

/// Round `round`'s mixing function of the working words and its constant.
fn mix(round: usize, second: u32, third: u32, fourth: u32) -> (u32, u32) {
  match round {
    0..=19 => ((second & third) | (!second & fourth), 0x5A82_7999),
    20..=39 => (second ^ third ^ fourth, 0x6ED9_EBA1),
    40..=59 => (
      (second & third) | (second & fourth) | (third & fourth),
      0x8F1B_BCDC,
    ),
    _ => (second ^ third ^ fourth, 0xCA62_C1D6),
  }
}

fn compress(state: &mut [u32; 5], block: &[u8; 64]) {
  let mut words: Vec<u32> = block
    .as_chunks::<4>()
    .0
    .iter()
    .map(|bytes| u32::from_be_bytes(*bytes))
    .collect();
  for at in 16..80 {
    let mixed =
      word(&words, at - 3) ^ word(&words, at - 8) ^ word(&words, at - 14) ^ word(&words, at - 16);
    words.push(mixed.rotate_left(1));
  }
  let mut work = *state;
  for (round, value) in words.iter().enumerate() {
    let [first, second, third, fourth, fifth] = work;
    let (mixed, constant) = mix(round, second, third, fourth);
    let next = first
      .rotate_left(5)
      .wrapping_add(mixed)
      .wrapping_add(fifth)
      .wrapping_add(constant)
      .wrapping_add(*value);
    work = [next, first, second.rotate_left(30), third, fourth];
  }
  for (slot, add) in state.iter_mut().zip(work) {
    *slot = slot.wrapping_add(add);
  }
}

#[cfg(test)]
#[path = "tests/sha1_test.rs"]
mod tests;
