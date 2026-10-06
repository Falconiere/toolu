//! Word resolution: static values, quoting forms, pathname patterns, brace
//! expansion, and words tree-sitter splits that bash keeps whole.

use super::literal;
use crate::analysis::ShellCommand;
use crate::analyze;

fn last(source: &str) -> ShellCommand {
  analyze(source).commands.pop().unwrap()
}

#[test]
fn every_quoting_form_resolves_to_its_value() {
  let command = last("echo 'a b' \"c d\" $'e\\tf' $\"g\" a\\ b x\"y\"'z'");
  let words: Vec<Option<&str>> = command.words.iter().map(Option::as_deref).collect();
  assert_eq!(
    words,
    [
      Some("echo"),
      Some("a b"),
      Some("c d"),
      Some("e\tf"),
      Some("g"),
      Some("a b"),
      Some("xyz")
    ]
  );
}

#[test]
fn an_expansion_makes_the_word_dynamic_but_keeps_its_text() {
  let command = last("echo \"$HOME/a b\" ${x:-y} $((1+2)) {1..3} `date` <(x)");
  assert!(command.words.iter().skip(1).all(Option::is_none));
  assert_eq!(command.texts[1], "$HOME/a b");
  assert_eq!(command.texts[2], "${x:-y}");
  assert_eq!(command.texts[4], "{1..3}");
}

#[test]
fn a_pathname_pattern_is_a_pattern_not_a_value() {
  let command = last("ls x*.ts .en[v] 'q*'");
  assert_eq!(command.words[1], None);
  assert_eq!(command.patterns[1].as_deref(), Some("x*.ts"));
  assert_eq!(command.patterns[2].as_deref(), Some(".en[v]"));
  assert_eq!(command.words[3].as_deref(), Some("q*"));
}

#[test]
fn brace_expansion_is_dynamic() {
  let command = last("echo a{b,c}d {} {a} a,b {,} \\{x,y\\}");
  let words: Vec<Option<&str>> = command.words.iter().map(Option::as_deref).collect();
  assert_eq!(
    words,
    [
      Some("echo"),
      None,
      Some("{}"),
      Some("{a}"),
      Some("a,b"),
      None,
      Some("{x,y}")
    ]
  );
}

#[test]
fn a_word_split_at_an_escaped_separator_is_one_word() {
  assert_eq!(last("node -\\\ne x").words[1].as_deref(), Some("-e"));
  assert_eq!(last("echo 'a'\\ 'b'").words[1].as_deref(), Some("a b"));
  assert_eq!(last("echo 'a'\\ \\ 'b'").words[1].as_deref(), Some("a  b"));
  assert_eq!(last("echo 'a' 'b'").words.len(), 3);
}

#[test]
fn an_assignment_word_resolves_name_and_value() {
  let command = last("export A=$(x) B=2 C=\"d e\"");
  let words: Vec<Option<&str>> = command.words.iter().map(Option::as_deref).collect();
  assert_eq!(words, [Some("export"), None, Some("B=2"), Some("C=d e")]);
}

#[test]
fn a_word_with_no_node_is_an_unquoted_literal() {
  let dash = literal("-");
  assert_eq!(
    (dash.value.as_deref(), dash.pattern, dash.text.as_str()),
    (Some("-"), None, "-")
  );
  assert_eq!(literal("*.x").pattern.as_deref(), Some("*.x"));
}
