use std::ffi::OsString;
use std::path::Path;

use super::{NONCE_FILE, create, create_push, git, head, nonce, remote_has};
use crate::host_smoke_env::ChildEnv;

fn env(base: &Path) -> ChildEnv {
  let path = std::env::var_os("PATH").unwrap_or_else(|| OsString::from("/usr/bin:/bin"));
  ChildEnv::new(Path::new("/x/toolu"), &path, &base.join("shims")).unwrap()
}

#[test]
fn a_project_is_committed_with_the_gate_config_and_protected_env() {
  let root = tempfile::tempdir().unwrap();
  let env = env(root.path());
  let path = create(root.path(), "claude project with spaces", "claude", &env).unwrap();
  assert_eq!(
    std::fs::read_to_string(path.join(".env")).unwrap(),
    "KEEP=1\n"
  );
  let config = std::fs::read_to_string(path.join(".claude/toolu.config.json")).unwrap();
  assert!(config.contains("\"pushReview\"") && config.contains("\"maxFileLines\":3"));
  assert_eq!(git(&path, &env, &["status", "--porcelain"]).unwrap(), "");
  assert_eq!(
    git(&path, &env, &["branch", "--show-current"])
      .unwrap()
      .trim(),
    "main"
  );
  assert_eq!(head(&path, &env).unwrap().len(), 40);
}

#[test]
fn the_push_project_is_ahead_of_main_with_an_empty_bare_origin() {
  let root = tempfile::tempdir().unwrap();
  let env = env(root.path());
  let pushed = create_push(root.path(), "codex", &env).unwrap();
  let branch = git(&pushed.project, &env, &["branch", "--show-current"]).unwrap();
  assert_eq!(branch.trim(), "feat/push-smoke");
  let ahead = git(
    &pushed.project,
    &env,
    &["diff", "--name-only", "main...HEAD"],
  )
  .unwrap();
  assert_eq!(ahead.trim(), "note.txt");
  let origin = git(&pushed.project, &env, &["remote", "get-url", "origin"]).unwrap();
  assert_eq!(origin.trim(), pushed.remote.display().to_string());
  assert!(!remote_has(&pushed.remote, &env, "refs/heads/feat/smoke").unwrap());
}

#[test]
fn remote_has_sees_a_pushed_branch() {
  let root = tempfile::tempdir().unwrap();
  let env = env(root.path());
  let pushed = create_push(root.path(), "claude", &env).unwrap();
  git(
    &pushed.project,
    &env,
    &["push", "-q", "origin", "HEAD:refs/heads/feat/smoke"],
  )
  .unwrap();
  assert!(remote_has(&pushed.remote, &env, "refs/heads/feat/smoke").unwrap());
}

#[test]
fn git_reports_a_failing_command() {
  let root = tempfile::tempdir().unwrap();
  let env = env(root.path());
  assert!(git(root.path(), &env, &["rev-parse", "HEAD"]).is_err());
}

#[test]
fn each_project_holds_its_own_committed_nonce() {
  let root = tempfile::tempdir().unwrap();
  let env = env(root.path());
  let first = create(root.path(), "first", "claude", &env).unwrap();
  let second = create(root.path(), "second", "claude", &env).unwrap();
  let (one, two) = (nonce(&first).unwrap(), nonce(&second).unwrap());
  assert!(one.len() >= 20 && one.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
  assert_ne!(one, two);
  let file = std::fs::read_to_string(first.join(NONCE_FILE)).unwrap();
  assert_eq!(file, format!("{one}\n"));
  assert_eq!(git(&first, &env, &["status", "--porcelain"]).unwrap(), "");
}

#[test]
fn a_project_without_a_nonce_file_is_an_error() {
  let root = tempfile::tempdir().unwrap();
  assert!(nonce(root.path()).unwrap_err().contains("cannot read"));
}
