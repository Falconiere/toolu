use super::check;
use crate::doctor::checks::Status;
use toolu_runtime::env::Env;

#[test]
fn rejects_a_retired_selected_hc_without_fallback() {
  let env = Env::from_pairs([
    ("PATH", "/nonexistent"),
    ("TOOLU_EPIC_MULTIPLEXER", "herdr-app"),
  ]);
  let result = check(&env);
  assert_eq!(result.status, Status::Fail);
  assert!(result.summary.contains("herdr-app is unsupported"));
  assert!(result.summary.contains("auto, herdr or tmux"));
}

#[test]
fn invalid_selection_fails_the_check() {
  let env = Env::from_pairs([("TOOLU_EPIC_MULTIPLEXER", "typo")]);
  assert_eq!(check(&env).status, Status::Fail);
}
