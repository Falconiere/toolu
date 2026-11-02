use super::*;

#[test]
fn marker_must_match_live_config() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let store = Store::new(root);
  assert!(require(root).is_err());
  let config = config::Config {
    relay: Some("relay".into()),
    generation: Some("first".into()),
    ..Default::default()
  };
  enroll(root, &store, &config::Config::default(), config).unwrap();
  require(root).unwrap();
  std::fs::write(root.join("authority.json"), b"{\"machine\":\"other\"}").unwrap();
  assert!(require(root).is_err());
}

fn active(root: &Path, store: &Store) -> config::Config {
  let next = config::Config {
    relay: Some("relay".into()),
    generation: Some("first".into()),
    ..Default::default()
  };
  enroll(root, store, &config::Config::default(), next.clone()).unwrap();
  next
}

#[test]
fn stale_configuration_cannot_overwrite_newer_changes() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let original = active(dir.path(), &store);
  let mut changed = original.clone();
  changed
    .recipes
    .insert("origin".into(), vec![vec!["cargo".into(), "test".into()]]);
  configure(dir.path(), &store, &original, changed.clone()).unwrap();
  let mut stale = original.clone();
  stale
    .recipes
    .insert("other".into(), vec![vec!["make".into()]]);
  assert!(configure(dir.path(), &store, &original, stale).is_err());
  assert_eq!(config::load(dir.path()).unwrap(), changed);
}

#[test]
fn retirement_fences_old_marker_and_requires_new_generation_to_reenroll() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let store = Store::new(root);
  let original = active(root, &store);
  let marker = std::fs::read(root.join("authority.json")).unwrap();
  retire(root, &store, &original, "next").unwrap();
  std::fs::write(root.join("authority.json"), &marker).unwrap();
  assert!(require(root).is_err());
  assert!(configure(root, &store, &original, original.clone()).is_err());
  retire(root, &store, &original, "next").unwrap();
  let retired = config::load(root).unwrap();
  assert!(enroll(root, &store, &retired, original.clone()).is_err());
  let mut next = original.clone();
  next.generation = Some("second".into());
  enroll(root, &store, &retired, next.clone()).unwrap();
  enroll(root, &store, &retired, next.clone()).unwrap();
  require(root).unwrap();
  retire(root, &store, &original, "next").unwrap();
  assert_eq!(config::load(root).unwrap(), next);
  require(root).unwrap();
  retire(root, &store, &next, "third").unwrap();
  let retired_twice = config::load(root).unwrap();
  assert!(enroll(root, &store, &retired_twice, original).is_err());
}

#[test]
fn retirement_intent_fences_crash_before_marker_removal() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let store = Store::new(root);
  let original = active(root, &store);
  durable::write(
    &root.join("retired.json"),
    br#"{"generation":"first","successor":"next"}"#,
  )
  .unwrap();
  assert!(root.join("authority.json").exists());
  assert!(require(root).is_err());
  assert!(configure(root, &store, &original, original.clone()).is_err());
  retire(root, &store, &original, "next").unwrap();
  assert!(!root.join("authority.json").exists());
  assert!(config::load(root).unwrap().relay.is_none());
}
