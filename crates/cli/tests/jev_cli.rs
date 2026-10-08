//! The built `toolu` binary with no Jev key: exit 1, the unset line, no request.

use std::process::Command;

#[test]
fn a_missing_key_exits_1_and_prints_nothing_else() {
  let output = Command::new(env!("CARGO_BIN_EXE_toolu"))
    .args(["jev", "choice", "q", "-s", "STATE", "-o", "a", "-o", "b"])
    .env_remove("TYPESAFE_API_KEY")
    .output()
    .expect("spawn toolu");
  assert_eq!(output.status.code(), Some(1));
  assert!(output.stdout.is_empty(), "{:?}", output.stdout);
  assert_eq!(output.stderr, b"jev: TYPESAFE_API_KEY unset\n");
}
