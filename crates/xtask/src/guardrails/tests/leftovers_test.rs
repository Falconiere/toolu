use super::check;
use crate::guardrails::tests::{context, tree};

#[test]
fn markers_need_an_issue_number_on_their_line() {
  let source = "//! d\n// TODO tidy\n/* line one\n   FIXME: later */\n// TODO(#455) fine\n/// FIXME #12 fine\nconst S: &str = \"TODO in a string is not a comment\";\n// TODOS and FIXMEd are other words\n";
  let found = check(&context(
    &tree(&[("crates/demo/src/lib.rs", source)]).workspace,
  ))
  .unwrap();
  let lines: Vec<(usize, &str)> = found.iter().map(|f| (f.line, f.message.as_str())).collect();
  assert_eq!(
    lines,
    [
      (
        2,
        "TODO without an issue number — write TODO(#<issue>) or do it now"
      ),
      (
        4,
        "FIXME without an issue number — write FIXME(#<issue>) or do it now"
      ),
    ]
  );
}
