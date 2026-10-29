use super::{command, show};
use toolu_runtime::env::Env;

#[test]
fn requires_an_explicit_target() {
  assert!(
    command()
      .try_get_matches_from(["herdr-app", "show"])
      .is_err()
  );
  let matches = command()
    .try_get_matches_from(["herdr-app", "show", "vps/t7"])
    .unwrap();
  assert_eq!(
    matches
      .subcommand()
      .unwrap()
      .1
      .get_one::<String>("terminal-address")
      .unwrap(),
    "vps/t7"
  );
}

#[test]
fn missing_core_and_invalid_addresses_fail_without_opening_app() {
  let env = Env::from_pairs([("PATH", "/nonexistent")]);
  assert!(show(&env, "t7").unwrap_err().contains("hc transport"));
  assert!(show(&env, "").is_err());
  assert!(show(&env, "--help").is_err());
}
