use super::{is_number_text, jq_number};

#[test]
fn jq_numbers_parse_with_their_javascript_values() {
  let cases = [
    (" 5 ", 5.0),
    ("+.5", 0.5),
    ("5.", 5.0),
    ("007", 7.0),
    ("-1.5e3", -1500.0),
    ("\t2E-1\n", 0.2),
  ];
  for (text, number) in cases {
    assert_eq!(jq_number(text), Some(number), "{text:?}");
  }
}

#[test]
fn anything_else_is_not_a_number() {
  for text in [
    "", "abc", ".", "+", "1e", "1e+", "0x10", "1_000", "1e999", " 1 2", "١",
  ] {
    assert_eq!(jq_number(text), None, "{text:?}");
  }
}

#[test]
fn the_grammar_takes_no_surrounding_whitespace() {
  assert!(is_number_text("12.5e-3"));
  assert!(!is_number_text(" 1"));
  assert!(!is_number_text("1 "));
  assert!(!is_number_text("Infinity"));
}
