use toolu_shell::analyze;

use super::{commit_prefix, commits, unknown_prefix};

#[test]
fn subject_prefix_accepts_a_scoped_type() {
  assert_eq!(commit_prefix("feat(api): add x\nbody"), Some("feat"));
  assert_eq!(commit_prefix("Feat: add x"), None);
  assert_eq!(commit_prefix("feat api: add x"), None);
}

#[test]
fn static_message_is_checked_but_a_dynamic_one_is_not_guessed() {
  let prefixes = vec!["feat".to_owned(), "fix".to_owned()];
  let static_line = analyze("sudo git commit -m 'wibble: x'");
  assert_eq!(
    unknown_prefix(&static_line, &prefixes).as_deref(),
    Some("wibble")
  );
  let dynamic_line = analyze("git commit -m \"$MSG\"");
  assert!(commits(&dynamic_line));
  assert_eq!(unknown_prefix(&dynamic_line, &prefixes), None);
}

#[test]
fn an_unknown_line_names_git_and_commit_as_words() {
  let oversize = format!("git commit {}", "x".repeat(toolu_shell::MAX_SHELL_INPUT));
  assert!(commits(&analyze(&oversize)));
  let lookalike = format!("mygit commit {}", "x".repeat(toolu_shell::MAX_SHELL_INPUT));
  assert!(!commits(&analyze(&lookalike)));
}
