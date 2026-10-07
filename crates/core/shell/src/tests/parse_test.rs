//! The parser, its shared deadline, and syntax errors.

use std::time::Duration;

use super::{ParseFailure, Syntax, syntax_errors, utf16_len};
use crate::analysis::CommandOrigin;

fn errors(source: &str) -> Vec<(String, CommandOrigin)> {
  let mut syntax = Syntax::new(Duration::from_secs(5)).unwrap();
  let (tree, text) = syntax.script(source).unwrap();
  let found = syntax_errors(&tree, &text, CommandOrigin::Line);
  found
    .into_iter()
    .map(|error| (error.message, error.origin))
    .collect()
}

#[test]
fn utf16_length_counts_as_typescript_does() {
  assert_eq!(utf16_len("abc"), 3);
  assert_eq!(utf16_len("é"), 1);
  assert_eq!(utf16_len("😀"), 2);
}

#[test]
fn missing_and_error_nodes_become_messages() {
  assert_eq!(errors("git push"), Vec::<(String, CommandOrigin)>::new());
  assert_eq!(errors("echo 'x")[0].0, "unterminated single quote");
  let substitution = errors("echo $(x");
  assert!(
    substitution
      .iter()
      .any(|(message, _)| message.starts_with("expected"))
  );
  assert!(errors(")")[0].0.starts_with("syntax error near"));
  assert!(
    errors("if")
      .iter()
      .all(|(_, origin)| *origin == CommandOrigin::Line)
  );
}

#[test]
fn errors_inside_a_substitution_belong_to_it() {
  let found = errors("echo $(a &&)");
  assert!(
    found
      .iter()
      .any(|(_, origin)| *origin == CommandOrigin::Substitution),
    "{found:?}"
  );
}

#[test]
fn a_spent_budget_cancels_every_parse() {
  let mut syntax = Syntax::new(Duration::ZERO).unwrap();
  assert_eq!(
    syntax.script("git push").map(|_| ()),
    Err(ParseFailure::Cancelled)
  );
  assert!(ParseFailure::Cancelled.message().contains("parse budget"));
  assert_eq!(
    ParseFailure::Language("abi".to_owned()).message(),
    "parser: abi"
  );
}

#[test]
fn heredoc_state_past_the_scanner_buffer_parses_with_the_vendored_fix() {
  // 0.23.3's serializer overran its buffer here; the vendored fix stops short.
  let mut syntax = Syntax::new(Duration::from_secs(5)).unwrap();
  for source in [
    format!("{}\n", "cat <<EOF ".repeat(150)),
    format!("cat <<{}\nx\n", "A<".repeat(507)),
  ] {
    assert!(syntax.script(&source).is_ok(), "{:?}", source.get(..20));
  }
  assert_eq!(
    ParseFailure::Worker("no threads".to_owned()).message(),
    "parser: no worker thread: no threads"
  );
}
