use super::*;

#[test]
fn unfinished_selection_is_not_silently_retargeted() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", dir.path().to_str().unwrap())]);
  let pending = Pending {
    source: None,
    destination: "alpha".into(),
    expected_destination: config::Config::default(),
    next: config::Config {
      relay: Some("alpha".into()),
      generation: Some("generation".into()),
      ..config::Config::default()
    },
  };
  write(&path(&env), &pending).unwrap();
  let original = std::fs::read(path(&env)).unwrap();
  assert!(resume(&env, "beta").unwrap_err().contains("alpha"));
  assert_eq!(std::fs::read(path(&env)).unwrap(), original);
}
