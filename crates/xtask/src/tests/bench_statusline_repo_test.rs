use std::path::Path;

use super::{Fixture, payload};

#[test]
fn the_fixture_is_a_linked_worktree_with_an_untracked_file_and_is_removed_on_drop() {
  let fixture = Fixture::new().unwrap();
  let worktree = fixture.worktree.clone();
  assert!(
    worktree.join(".git").is_file(),
    "a linked worktree has a .git file"
  );
  assert!(worktree.join("untracked.txt").is_file());
  assert!(fixture.cfg.is_dir() && fixture.cold_cfg.is_dir());
  let root = fixture.cfg.parent().unwrap().to_path_buf();
  drop(fixture);
  assert!(!root.exists());
}

#[test]
fn the_env_drops_host_keys_and_points_home_config_and_path_at_the_sandbox() {
  let fixture = Fixture::new().unwrap();
  let env = fixture.env(Path::new("/opt/toolu/bin/toolu"), &fixture.cold_cfg);
  let get = |key: &str| {
    env
      .iter()
      .rev()
      .find(|(k, _)| k == key)
      .map(|(_, v)| v.clone())
  };
  assert_eq!(
    get("CLAUDE_CONFIG_DIR"),
    Some(fixture.cold_cfg.display().to_string())
  );
  assert_eq!(
    get("TYPESAFE_API_KEY").as_deref(),
    Some("statusline-benchmark-key")
  );
  assert!(get("PATH").unwrap().starts_with("/opt/toolu/bin:"));
  assert_eq!(env.iter().filter(|(k, _)| k == "HOME").count(), 1);
  assert_ne!(get("HOME").unwrap(), "");
}

#[test]
fn the_payload_points_the_workspace_at_the_worktree_and_needs_one() {
  let text = payload(
    r#"{"workspace": {"current_dir": "$REPO"}, "a": 1}"#,
    Path::new("/w"),
  )
  .unwrap();
  let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
  assert_eq!(doc["workspace"]["current_dir"], "/w");
  assert_eq!(doc["a"], 1);
  let err = payload("{}", Path::new("/w")).unwrap_err();
  assert!(err.contains("no workspace"), "{err}");
  assert!(payload("not json", Path::new("/w")).is_err());
}
