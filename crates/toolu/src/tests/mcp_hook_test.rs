use super::*;

#[test]
fn malformed_payload_is_silent() {
  assert_eq!(
    evaluate("[1]", None, Env::process(), Path::new(".")),
    silent()
  );
}
