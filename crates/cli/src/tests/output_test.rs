use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;

use super::emit;

#[test]
fn both_streams_can_always_be_written() {
  emit(&Outcome {
    exit: Exit::Success,
    stdout: Some("out".to_owned()),
    stderr: Some("err".to_owned()),
  });
  emit(&Outcome {
    exit: Exit::Usage,
    stdout: None,
    stderr: None,
  });
}
