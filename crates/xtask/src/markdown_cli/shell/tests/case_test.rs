use crate::markdown_cli::shell::tests::{names, owned};

#[test]
fn case_patterns_run_nothing_and_their_arms_run_commands() {
  let text = "case \"$1\" in\n  foo) gh pr view ;;\n  bar|baz)\n    git push\n    ;;\n  *) exit 1 ;;\nesac\nls\n";
  assert_eq!(
    names(text),
    owned(&[("gh", 2), ("git", 4), ("exit", 6), ("ls", 8)])
  );
}

#[test]
fn a_semicolon_outside_a_case_only_ends_the_command() {
  assert_eq!(names("gh a;; git b\n"), owned(&[("gh", 1), ("git", 1)]));
  assert_eq!(names("case x in\n  esac\nls\n"), owned(&[("ls", 3)]));
}

#[test]
fn a_comment_between_arms_is_not_a_pattern() {
  let text = "case $x in\n  # (see above)\n  foo) gh a ;;\nesac\n";
  assert_eq!(names(text), owned(&[("gh", 3)]));
}
