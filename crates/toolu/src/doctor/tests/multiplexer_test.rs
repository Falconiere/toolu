use super::check;
use crate::doctor::checks::Status;
use toolu_runtime::env::Env;

#[test]
fn reports_a_missing_selected_hc_without_fallback() {
  let env = Env::from_pairs([
    ("PATH", "/nonexistent"),
    ("TOOLU_EPIC_MULTIPLEXER", "herdr-app"),
  ]);
  let result = check(&env);
  assert_eq!(result.status, Status::Warn);
  assert_eq!(result.details["backend"], "herdr-app");
  assert_eq!(result.details["available"], false);
}

#[test]
fn invalid_selection_fails_the_check() {
  let env = Env::from_pairs([("TOOLU_EPIC_MULTIPLEXER", "typo")]);
  assert_eq!(check(&env).status, Status::Fail);
}
