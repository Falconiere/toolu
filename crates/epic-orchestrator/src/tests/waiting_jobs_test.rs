use std::time::{Duration, UNIX_EPOCH};

use toolu_engine::resources::binding::ResourceBinding;

use super::{clear, list, mark};

#[test]
fn resource_status_waiting_marker_preserves_first_wait_and_expires() {
  let temp = tempfile::tempdir().expect("resource root");
  let binding = ResourceBinding {
    root: temp.path().to_path_buf(),
    key: "repo-449".to_owned(),
    state_dir: temp.path().join("epic-402").display().to_string(),
    worktree: temp.path().join("worktree"),
  };
  let first = UNIX_EPOCH + Duration::from_secs(1_000);
  mark(&binding, first).expect("first refusal");
  mark(&binding, first + Duration::from_secs(5)).expect("second refusal");
  let rows = list(temp.path(), 1_007_000).expect("waiting jobs");
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0]["key"], "repo-449");
  assert_eq!(rows[0]["age_ms"], 7_000);
  assert_eq!(
    list(temp.path(), 1_130_000).expect("stale"),
    Vec::<serde_json::Value>::new()
  );
  clear(&binding);
  assert_eq!(
    list(temp.path(), 1_007_000).expect("cleared"),
    Vec::<serde_json::Value>::new()
  );
}
