use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;

use super::printed;

#[test]
fn recorded_output_includes_both_blocking_streams() {
  let outcome = Outcome {
    exit: Exit::Failure,
    stdout: Some("summary".to_owned()),
    stderr: Some("tree changed during the run".to_owned()),
  };
  assert_eq!(printed(&outcome), "summary\ntree changed during the run\n");
}
