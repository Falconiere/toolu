use super::*;

#[test]
fn pinned_endpoint_survives_configuration_change() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([
    ("TOOLU_CONFIG_DIR", dir.path().to_str().unwrap()),
    ("TOOLU_RELAY_TARGET", "first"),
  ]);
  assert_eq!(pin(&env, "task").unwrap(), "first");
  let changed = env.with("TOOLU_RELAY_TARGET", "second");
  assert_eq!(pin(&changed, "task").unwrap(), "first");
  assert_eq!(get(&changed, "task").unwrap(), "first");
  assert_eq!(get(&changed, "unknown").unwrap(), "second");
  assert!(get(&changed, "../escape").is_err());
}
