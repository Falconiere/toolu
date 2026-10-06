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
fn heredoc_state_counts_operators_and_delimiters() {
  assert_eq!(super::heredoc_state("git push"), (0, 4));
  assert_eq!(
    super::heredoc_state("cat <<EOF >x <<-'END'"),
    (2, 4 + 10 + 12)
  );
  assert_eq!(super::heredoc_state("cat <<< x"), (0, 4));
}

#[test]
fn more_heredoc_state_than_the_scanner_can_hold_is_not_parsed() {
  let mut syntax = Syntax::new(Duration::from_secs(5)).unwrap();
  let pending = format!("{}\n", "cat <<EOF ".repeat(150));
  assert_eq!(
    syntax.script(&pending).map(|_| ()),
    Err(ParseFailure::Heredocs(150))
  );
  let long = format!("cat <<{}\nx\n", "A".repeat(1_100));
  assert_eq!(
    syntax.script(&long).map(|_| ()),
    Err(ParseFailure::Heredocs(1))
  );
  assert!(ParseFailure::Heredocs(3).message().contains("3 heredocs"));
}
