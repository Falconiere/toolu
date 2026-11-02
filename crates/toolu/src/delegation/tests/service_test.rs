use super::*;

#[test]
fn relay_names_are_validated_before_configuration_is_written() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", dir.path().to_str().unwrap())]);
  assert!(select(&env, "../bad").is_err());
  assert!(select(&env, "").is_err());
  assert!(!config::root(&env).exists());
  assert!(serve(&env).is_err());
}
