use toolu_runtime::env::Env;

use super::{CleanupJob, cleanup};
use crate::paths::Paths;
use crate::reach::Reach;

#[test]
fn a_missing_record_is_cleanup_incomplete_with_the_token_redacted() {
  let tmp = tempfile::tempdir().expect("temp");
  let state = tmp.path().join("ghp-sentinel-dir");
  std::fs::create_dir_all(state.join("issues")).expect("issues");
  let env = Env::from_pairs([
    ("GH_TOKEN", "ghp-sentinel"),
    ("HOME", &tmp.path().display().to_string()),
  ]);
  let outcome = cleanup(&CleanupJob {
    state_dir: state,
    key: "k-1".into(),
    env,
    paths: Paths::at(tmp.path()),
    reach: Reach::live(),
  });
  assert_eq!(outcome["outcome"], "cleanup-incomplete");
  let note = outcome["note"].as_str().expect("note");
  assert!(!note.contains("ghp-sentinel"), "{note}");
  assert!(note.contains("[redacted]"), "{note}");
}
