use toolu_protocol::exit::Exit;

use super::*;

#[test]
fn malformed_json_is_rejected_before_any_state_write() {
  let parsed = crate::command()
    .try_get_matches_from([
      "review",
      "write-state",
      "--findings-count",
      "0",
      "--findings",
      "[bad",
    ])
    .unwrap();
  let (_, verb) = parsed.subcommand().unwrap();
  let result = input(verb);
  assert!(matches!(
    result,
    Err(Outcome {
      exit: Exit::Failure,
      ..
    })
  ));
}
