use super::self_usage;

#[test]
fn peak_resident_memory_grows_with_touched_pages_and_cpu_never_goes_back() {
  let before = self_usage().unwrap();
  let mut block = vec![0_u8; 32 * 1024 * 1024];
  for (index, byte) in block.iter_mut().enumerate().step_by(4096) {
    *byte = u8::try_from(index % 251).unwrap();
  }
  let mut spin = 0_u64;
  for value in 0..2_000_000_u64 {
    spin = spin.wrapping_add(value ^ u64::from(block[usize::try_from(value % 1024).unwrap()]));
  }
  let after = self_usage().unwrap();
  assert!(spin > 0);
  assert!(
    after.max_rss_bytes >= before.max_rss_bytes.max(32 * 1024 * 1024),
    "{before:?} {after:?}"
  );
  assert!(after.cpu_ms >= before.cpu_ms, "{before:?} {after:?}");
  drop(block);
}

#[test]
fn rss_units_are_bytes_on_every_platform() {
  assert_eq!(
    super::to_bytes(10),
    if cfg!(target_os = "macos") {
      10
    } else {
      10 * 1024
    }
  );
  assert_eq!(super::to_bytes(-1), 0);
}
