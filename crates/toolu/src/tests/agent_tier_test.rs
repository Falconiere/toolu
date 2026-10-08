use super::*;

#[test]
fn malformed_payload_fails_open() {
  let out = evaluate("{", Env::process(), Path::new("."));
  assert_eq!(out, silent());
}
