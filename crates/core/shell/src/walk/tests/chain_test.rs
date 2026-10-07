//! Flattening a statement into an and-or list of pipelines.

use super::chain;
use crate::PARSE_BUDGET;
use crate::parse::Syntax;

/// `(pipeline sizes, negated flags, operators, redirects on each last element)`.
fn shape(source: &str) -> (Vec<usize>, Vec<bool>, Vec<&'static str>, Vec<usize>) {
  let mut syntax = Syntax::new(PARSE_BUDGET).unwrap();
  let (tree, text) = syntax.script(source).unwrap();
  let statement = tree.root_node().named_child(0).unwrap();
  let flat = chain(statement, &text);
  let sizes = flat.pipelines.iter().map(|p| p.elements.len()).collect();
  let negated = flat.pipelines.iter().map(|p| p.negated).collect();
  let attached = flat
    .pipelines
    .iter()
    .map(|p| p.elements.last().map_or(0, |e| e.redirects.len()))
    .collect();
  (sizes, negated, flat.operators, attached)
}

#[test]
fn a_left_recursive_list_flattens_in_order() {
  let (sizes, _, operators, _) = shape("a && b || c && d");
  assert_eq!(
    (sizes, operators),
    (vec![1, 1, 1, 1], vec!["&&", "||", "&&"])
  );
}

#[test]
fn pipelines_group_between_and_or_operators() {
  let (sizes, negated, operators, _) = shape("! a | b && c |& d");
  assert_eq!(
    (sizes, negated, operators),
    (vec![2, 2], vec![true, false], vec!["&&"])
  );
}

#[test]
fn a_redirect_on_a_list_goes_to_its_last_command() {
  let (sizes, _, operators, attached) = shape("a && b 2>&1 | c");
  assert_eq!(
    (sizes, operators, attached),
    (vec![1, 2], vec!["&&"], vec![0, 0])
  );
}

#[test]
fn a_heredoc_continuation_rejoins_the_line() {
  let (sizes, _, operators, _) = shape("cat <<EOF | grep x && git push\nbody\nEOF");
  assert_eq!((sizes, operators), (vec![2, 1], vec!["&&"]));
  let (sizes, _, operators, _) = shape("cat <<EOF || git push\nbody\nEOF");
  assert_eq!((sizes, operators), (vec![1, 1], vec!["||"]));
}

#[test]
fn a_missing_command_is_no_element() {
  let (sizes, _, _, _) = shape("git push |");
  assert_eq!(sizes, [1]);
}
