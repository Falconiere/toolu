use super::emit;
use crate::Outcome;

#[test]
fn both_streams_can_always_be_written() {
  emit(&Outcome {
    code: 0,
    stdout: Some("out".to_owned()),
    stderr: Some("err".to_owned()),
  });
  emit(&Outcome::default());
}
