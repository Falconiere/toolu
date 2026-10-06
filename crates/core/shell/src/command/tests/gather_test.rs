//! Gathering one command from tree-sitter's nodes.

use super::gather;
use crate::PARSE_BUDGET;
use crate::parse::Syntax;

/// The words (as resolved), redirect count and text end of the first command.
fn gathered(source: &str) -> (Vec<Option<String>>, usize, usize) {
  let mut syntax = Syntax::new(PARSE_BUDGET).unwrap();
  let (tree, text) = syntax.script(source).unwrap();
  let mut node = tree.root_node().named_child(0).unwrap();
  let mut statement = Vec::new();
  if node.kind() == "redirected_statement" {
    let mut cursor = node.walk();
    statement = node
      .children_by_field_name("redirect", &mut cursor)
      .collect();
    node = node.child_by_field_name("body").unwrap();
  }
  let found = gather(node, &statement, &text);
  let words = found
    .words
    .iter()
    .map(|word| word.resolve(&text).value)
    .collect();
  (words, found.redirects.len(), found.end)
}

fn some(list: &[&str]) -> Vec<Option<String>> {
  list.iter().map(|word| Some((*word).to_owned())).collect()
}

#[test]
fn heredoc_arguments_and_redirects_fold_into_the_command() {
  let (words, redirects, end) = gathered("cat <<EOF a >f b\nx\nEOF");
  assert_eq!(words, some(&["cat", "a", "b"]));
  assert_eq!(redirects, 2);
  assert_eq!(end, "cat <<EOF a >f b".len());
}

#[test]
fn a_lost_dash_is_restored_in_order() {
  let (words, _, _) = gathered("python3 - <<'EOF'\nx\nEOF");
  assert_eq!(words, some(&["python3", "-"]));
}

#[test]
fn hollow_words_are_dropped() {
  let mut syntax = Syntax::new(PARSE_BUDGET).unwrap();
  let (tree, text) = syntax.script("x &&").unwrap();
  let list = tree.root_node().named_child(0).unwrap();
  let hollow = list.named_child(1).unwrap();
  assert!(gather(hollow, &[], &text).words.is_empty());
}
