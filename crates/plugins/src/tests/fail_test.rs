use toolu_protocol::exit::Exit;

use super::Fail;

#[test]
fn usage_and_missing_input_exit_64_and_failures_exit_1() {
  let usage = Fail::usage("remove requires --yes to confirm");
  assert_eq!(usage.exit, Exit::Usage);
  assert_eq!(usage.exit.code(), 64);
  assert_eq!(usage.message, "remove requires --yes to confirm");
  let failed = Fail::failed(String::from("cannot write /x"));
  assert_eq!(failed.exit.code(), 1);
  assert_eq!(failed.message, "cannot write /x");
}
