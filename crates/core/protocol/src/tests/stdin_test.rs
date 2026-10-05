use super::read_all;

#[test]
fn a_payload_reads_to_the_end() {
  let payload = br#"{"hook_event_name":"SessionStart","source":"startup"}"#;
  assert_eq!(
    read_all(&payload[..]).unwrap(),
    String::from_utf8_lossy(payload)
  );
}

#[test]
fn invalid_utf8_is_an_error() {
  assert!(read_all(&[0xff, 0xfe][..]).is_err());
}
