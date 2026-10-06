use super::{Command, command, is_assignment, unquote};

fn words(text: &str) -> Vec<(String, usize)> {
  text.split(' ').map(|word| (word.to_owned(), 4)).collect()
}

#[test]
fn leading_assignments_and_keywords_drop() {
  assert_eq!(
    command(words("then FOO=1 _x=y if gh pr view")),
    Some(Command {
      line: 4,
      words: vec!["gh".to_owned(), "pr".to_owned(), "view".to_owned()]
    })
  );
  assert_eq!(command(words("A=1 B=2")), None);
  assert_eq!(command(Vec::new()), None);
}

#[test]
fn closing_keywords_and_headers_run_nothing() {
  for text in [
    "fi",
    "done",
    "}",
    "for x in a b",
    "case $x in",
    "function setup",
  ] {
    assert_eq!(command(words(text)), None, "{text}");
  }
}

#[test]
fn assignments_need_a_name_before_the_equals_sign() {
  assert!(is_assignment("JEV_BUN="));
  assert!(is_assignment("x=$(mktemp)"));
  for word in ["--flag=x", "=x", "1x=y", "a-b=c", "plain"] {
    assert!(!is_assignment(word), "{word}");
  }
}

#[test]
fn unquote_drops_quote_characters() {
  assert_eq!(unquote("\"$S/route.ts\""), "$S/route.ts");
  assert_eq!(unquote("'<x>'"), "<x>");
}
