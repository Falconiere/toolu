use super::md5_hex;

#[test]
fn md5_matches_the_reference_vectors() {
  assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
  assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
  assert_eq!(
    md5_hex(b"The quick brown fox jumps over the lazy dog"),
    "9e107d9d372bb6826bd81d3542a419d6"
  );
  let long = "12345678901234567890123456789012345678901234567890123456789012345678901234567890";
  assert_eq!(md5_hex(long.as_bytes()), "57edf4a22be3c955ac49da2e2107b67a");
}
