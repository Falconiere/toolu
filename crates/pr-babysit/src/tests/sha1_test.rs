use super::hex;

#[test]
fn fips_180_vectors_hash_as_published() {
  assert_eq!(hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
  assert_eq!(hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
  assert_eq!(
    hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
    "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
  );
  assert_eq!(
    hex(&vec![b'a'; 1_000_000]),
    "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
  );
}

#[test]
fn inputs_around_the_padding_boundary_match_shasum() {
  assert_eq!(hex(&[b'x'; 55]), "cef734ba81a024479e09eb5a75b6ddae62e6abf1");
  assert_eq!(hex(&[b'x'; 56]), "901305367c259952f4e7af8323f480d59f81335b");
  assert_eq!(hex(&[b'x'; 64]), "bb2fa3ee7afb9f54c6dfb5d021f14b1ffe40c163");
}
