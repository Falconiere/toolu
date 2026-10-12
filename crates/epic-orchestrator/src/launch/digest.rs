//! MD5, only to name Cursor Agent's chat directory (`md5(resolved cwd)`); it
//! secures nothing.

use std::fmt::Write;

const SHIFTS: [u32; 64] = [
  7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14,
  20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6,
  10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

const SINES: [u32; 64] = [
  0xd76a_a478,
  0xe8c7_b756,
  0x2420_70db,
  0xc1bd_ceee,
  0xf57c_0faf,
  0x4787_c62a,
  0xa830_4613,
  0xfd46_9501,
  0x6980_98d8,
  0x8b44_f7af,
  0xffff_5bb1,
  0x895c_d7be,
  0x6b90_1122,
  0xfd98_7193,
  0xa679_438e,
  0x49b4_0821,
  0xf61e_2562,
  0xc040_b340,
  0x265e_5a51,
  0xe9b6_c7aa,
  0xd62f_105d,
  0x0244_1453,
  0xd8a1_e681,
  0xe7d3_fbc8,
  0x21e1_cde6,
  0xc337_07d6,
  0xf4d5_0d87,
  0x455a_14ed,
  0xa9e3_e905,
  0xfcef_a3f8,
  0x676f_02d9,
  0x8d2a_4c8a,
  0xfffa_3942,
  0x8771_f681,
  0x6d9d_6122,
  0xfde5_380c,
  0xa4be_ea44,
  0x4bde_cfa9,
  0xf6bb_4b60,
  0xbebf_bc70,
  0x289b_7ec6,
  0xeaa1_27fa,
  0xd4ef_3085,
  0x0488_1d05,
  0xd9d4_d039,
  0xe6db_99e5,
  0x1fa2_7cf8,
  0xc4ac_5665,
  0xf429_2244,
  0x432a_ff97,
  0xab94_23a7,
  0xfc93_a039,
  0x655b_59c3,
  0x8f0c_cc92,
  0xffef_f47d,
  0x8584_5dd1,
  0x6fa8_7e4f,
  0xfe2c_e6e0,
  0xa301_4314,
  0x4e08_11a1,
  0xf753_7e82,
  0xbd3a_f235,
  0x2ad7_d2bb,
  0xeb86_d391,
];

fn rounds(state: [u32; 4], chunk: &[u8; 64]) -> [u32; 4] {
  let words: Vec<u32> = chunk
    .as_chunks::<4>()
    .0
    .iter()
    .map(|bytes| u32::from_le_bytes(*bytes))
    .collect();
  let [mut a, mut b, mut c, mut d] = state;
  for (round, (sine, shift)) in SINES.iter().zip(SHIFTS).enumerate() {
    let (mix, index) = match round / 16 {
      0 => ((b & c) | (!b & d), round),
      1 => ((d & b) | (!d & c), (5 * round + 1) % 16),
      2 => (b ^ c ^ d, (3 * round + 5) % 16),
      _ => (c ^ (b | !d), (7 * round) % 16),
    };
    let word = words.get(index).copied().unwrap_or(0);
    let turned = a
      .wrapping_add(mix)
      .wrapping_add(*sine)
      .wrapping_add(word)
      .rotate_left(shift);
    (a, d, c) = (d, c, b);
    b = b.wrapping_add(turned);
  }
  let [s0, s1, s2, s3] = state;
  [
    s0.wrapping_add(a),
    s1.wrapping_add(b),
    s2.wrapping_add(c),
    s3.wrapping_add(d),
  ]
}

/// The lowercase hex MD5 of `input`.
pub(crate) fn md5_hex(input: &[u8]) -> String {
  let bits = u64::try_from(input.len())
    .unwrap_or(u64::MAX)
    .wrapping_mul(8);
  let mut message = input.to_vec();
  message.push(0x80);
  while message.len() % 64 != 56 {
    message.push(0);
  }
  message.extend_from_slice(&bits.to_le_bytes());
  let state = message
    .as_chunks::<64>()
    .0
    .iter()
    .fold([0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476], rounds);
  state
    .iter()
    .flat_map(|word| word.to_le_bytes())
    .fold(String::new(), |mut out, byte| {
      let _written = write!(out, "{byte:02x}");
      out
    })
}

#[cfg(test)]
#[path = "tests/digest_test.rs"]
mod tests;
