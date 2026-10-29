use std::path::Path;
use std::process::Command;

use super::dirty_except_state;
use toolu_runtime::env::Env;

fn init_repo(root: &Path) {
  assert!(
    Command::new("git")
      .args(["init", "-q"])
      .current_dir(root)
      .status()
      .unwrap()
      .success()
  );
}

#[test]
fn only_untracked_plugin_state_is_ignored() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  init_repo(root);
  let state = root.join(".codex/tmp");
  std::fs::create_dir_all(state.join("code-review")).unwrap();
  std::fs::create_dir_all(state.join("push-review")).unwrap();
  std::fs::create_dir_all(state.join("code-review-usage")).unwrap();
  std::fs::write(state.join("code-review/feature.json"), "{}").unwrap();
  std::fs::write(state.join("push-review/feature.json"), "{}").unwrap();
  std::fs::write(state.join("code-review-usage.jsonl"), "{}\n").unwrap();
  std::fs::write(state.join("code-review-usage.jsonl.lock"), "locked\n").unwrap();
  std::fs::write(
    state.join("code-review-usage/00000000000000000001.jsonl"),
    "{}\n",
  )
  .unwrap();
  let env = Env::process();
  assert!(!dirty_except_state(&env, root, &state).unwrap());
  let other = state.join("code-review-usage/notes.jsonl");
  std::fs::write(&other, "unrelated").unwrap();
  assert!(dirty_except_state(&env, root, &state).unwrap());
  std::fs::remove_file(other).unwrap();
  std::fs::write(root.join("user.txt"), "dirty").unwrap();
  assert!(dirty_except_state(&env, root, &state).unwrap());
}

#[test]
fn own_state_below_a_subdirectory_root_or_past_the_probe_cap_stays_clean() {
  let dir = tempfile::tempdir().unwrap();
  init_repo(dir.path());
  // Porcelain prints toplevel-relative paths even when the project root is a subdirectory.
  let root = dir.path().join("app");
  let state = root.join(".claude/tmp");
  std::fs::create_dir_all(state.join("code-review")).unwrap();
  // More own-state names than the 4 KB status probe holds.
  for index in 0..200 {
    let name = format!("code-review/feature-branch-number-{index}.json");
    std::fs::write(state.join(name), "{}").unwrap();
  }
  let env = Env::process();
  assert!(!dirty_except_state(&env, &root, &state).unwrap());
  std::fs::write(dir.path().join("top.txt"), "dirty").unwrap();
  assert!(dirty_except_state(&env, &root, &state).unwrap());
}

#[test]
fn archive_exclusion_treats_configured_state_root_literally() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  init_repo(root);
  let state = root.join("config[1]/tmp");
  let archives = state.join("code-review-usage");
  std::fs::create_dir_all(&archives).unwrap();
  std::fs::write(archives.join("00000000000000000001.jsonl"), "{}\n").unwrap();
  let env = Env::process();
  assert!(!dirty_except_state(&env, root, &state).unwrap());
  std::fs::write(archives.join("notes.jsonl"), "unrelated\n").unwrap();
  assert!(dirty_except_state(&env, root, &state).unwrap());
  std::fs::remove_file(archives.join("notes.jsonl")).unwrap();
  let overflow = archives.join("99999999999999999999.jsonl");
  std::fs::write(&overflow, "not a writer sequence\n").unwrap();
  assert!(dirty_except_state(&env, root, &state).unwrap());
  std::fs::remove_file(overflow).unwrap();
  assert!(
    Command::new("git")
      .arg("add")
      .arg("-A")
      .current_dir(root)
      .status()
      .unwrap()
      .success()
  );
  assert!(
    Command::new("git")
      .args([
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "-qm",
        "state"
      ])
      .current_dir(root)
      .status()
      .unwrap()
      .success()
  );
  assert!(!dirty_except_state(&env, root, &state).unwrap());
  std::fs::write(archives.join("00000000000000000001.jsonl"), "modified\n").unwrap();
  assert!(dirty_except_state(&env, root, &state).unwrap());
}
