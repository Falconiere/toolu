use std::process::Command;

use super::dirty_except_state;
use toolu_runtime::env::Env;

#[test]
fn only_untracked_plugin_state_is_ignored() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  assert!(
    Command::new("git")
      .arg("init")
      .arg("-q")
      .current_dir(root)
      .status()
      .unwrap()
      .success()
  );
  let state = root.join(".codex/tmp");
  std::fs::create_dir_all(state.join("code-review")).unwrap();
  std::fs::create_dir_all(state.join("push-review")).unwrap();
  std::fs::write(state.join("code-review/feature.json"), "{}").unwrap();
  std::fs::write(state.join("push-review/feature.json"), "{}").unwrap();
  std::fs::write(state.join("code-review-usage.jsonl"), "{}\n").unwrap();
  let env = Env::process();
  assert!(!dirty_except_state(&env, root, &state).unwrap());
  std::fs::write(root.join("user.txt"), "dirty").unwrap();
  assert!(dirty_except_state(&env, root, &state).unwrap());
}
