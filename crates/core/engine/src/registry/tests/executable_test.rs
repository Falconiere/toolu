use super::{NOT_RUN, TIMED_OUT, exit};

#[test]
fn a_module_that_did_not_finish_has_a_fixed_status() {
  assert_eq!((TIMED_OUT, NOT_RUN), (124, 127));
  let result = exit(NOT_RUN, "bash: not found\n".to_owned());
  assert_eq!((result.stdout.as_str(), result.exit_code), ("", 127));
}
