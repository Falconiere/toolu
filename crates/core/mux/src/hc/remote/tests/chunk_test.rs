use super::CHUNK;

#[test]
fn chunks_preserve_a_partial_last_block() {
  let data = vec![42_u8; CHUNK + 19];
  let chunks: Vec<&[u8]> = data.chunks(CHUNK).collect();
  assert_eq!(chunks.len(), 2);
  assert_eq!(chunks[1], &[42_u8; 19]);
}
