use super::{group, is};

pattern!(WORD, r"^(\w+) (\d+)$");
pattern!(BROKEN, &["(", "unclosed"].concat());

#[test]
fn a_pattern_matches_and_captures() {
  assert!(WORD.is_some());
  assert!(is(&WORD, "abc 12"));
  assert!(!is(&WORD, "abc"));
  assert_eq!(group(&WORD, "abc 12", 2).as_deref(), Some("12"));
  assert_eq!(group(&WORD, "abc 12", 3), None);
}

#[test]
fn a_broken_pattern_matches_nothing() {
  assert!(BROKEN.is_none());
  assert!(!is(&BROKEN, "(unclosed"));
  assert_eq!(group(&BROKEN, "(unclosed", 0), None);
}
