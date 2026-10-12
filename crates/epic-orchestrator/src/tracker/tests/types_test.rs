use super::{EXCERPT_CHARS, excerpt, repo_for, state_word};

#[test]
fn excerpt_cuts_on_characters_and_trims() {
  let text = format!("  {}😀😀  ", "a".repeat(EXCERPT_CHARS - 1));
  let out = excerpt(&text);
  assert_eq!(out.chars().count(), EXCERPT_CHARS);
  assert!(out.ends_with('😀'));
  assert_eq!(excerpt("  short\n"), "short");
}

#[test]
fn code_repo_comes_from_a_label_then_a_repo_line_then_the_default() {
  let labels = vec!["backend".to_owned(), "repo:acme/api".to_owned()];
  assert_eq!(repo_for(&labels, "Repo: acme/web", "acme/mono"), "acme/api");
  assert_eq!(
    repo_for(&[], "Context\nRepo: acme/web\n", "acme/mono"),
    "acme/web"
  );
  assert_eq!(
    repo_for(&["REPO:Acme/Api".to_owned()], "", "acme/mono"),
    "Acme/Api"
  );
  assert_eq!(repo_for(&[], "no hint", "acme/mono"), "acme/mono");
}

#[test]
fn state_word_names_open_and_closed() {
  assert_eq!(state_word(true), "closed");
  assert_eq!(state_word(false), "open");
}
