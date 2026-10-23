use std::os::unix::fs::PermissionsExt as _;

use super::{Host, hosts, require_auth, require_bin, require_platform, run, settle};
use crate::Verdict;
use crate::options::Options;

#[test]
fn host_defaults_to_all_three_and_names_one() {
  let all = vec![Host::Claude, Host::Codex, Host::Muse];
  assert_eq!(hosts(None).unwrap(), all);
  assert_eq!(hosts(Some("both")).unwrap(), all);
  assert_eq!(hosts(Some("claude")).unwrap(), vec![Host::Claude]);
  assert_eq!(hosts(Some("codex")).unwrap(), vec![Host::Codex]);
  assert_eq!(hosts(Some("muse")).unwrap(), vec![Host::Muse]);
  assert_eq!(hosts(Some("cursor")).unwrap(), vec![Host::Cursor]);
}

#[test]
fn an_unknown_host_is_an_error() {
  let err = hosts(Some("vim")).unwrap_err();
  assert_eq!(
    err,
    "--host must be claude, codex, muse, cursor or both, not vim"
  );
}

#[test]
fn muse_and_cursor_run_anywhere_and_claude_and_codex_need_macos() {
  assert_eq!(require_platform(&[Host::Muse], "linux"), Ok(()));
  assert_eq!(require_platform(&[Host::Cursor], "linux"), Ok(()));
  assert_eq!(require_platform(&[Host::Muse], "macos"), Ok(()));
  assert_eq!(
    require_platform(&[Host::Claude, Host::Codex], "macos"),
    Ok(())
  );
  for planned in [vec![Host::Claude], vec![Host::Codex], hosts(None).unwrap()] {
    assert_eq!(
      require_platform(&planned, "linux").unwrap_err(),
      "live Claude Code and Codex smoke requires macOS"
    );
  }
}

#[test]
fn run_refuses_an_unknown_host_before_anything_else() {
  let options = Options {
    host: Some("vim".to_owned()),
    ..Options::default()
  };
  assert!(run(&options).unwrap_err().contains("--host must be"));
}

#[test]
fn the_binary_must_exist_and_be_executable() {
  let dir = tempfile::tempdir().unwrap();
  let bin = dir.path().join("toolu");
  assert!(
    require_bin(&bin)
      .unwrap_err()
      .contains("native binary missing")
  );
  std::fs::write(&bin, "#!/bin/sh\n").unwrap();
  std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o644)).unwrap();
  assert!(require_bin(&bin).unwrap_err().contains("not an executable"));
  std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
  assert_eq!(
    require_bin(&bin).unwrap(),
    std::fs::canonicalize(&bin).unwrap()
  );
}

#[test]
fn codex_auth_is_the_users_auth_json() {
  let home = tempfile::tempdir().unwrap();
  assert!(
    require_auth(home.path())
      .unwrap_err()
      .contains("no Codex auth")
  );
  std::fs::create_dir_all(home.path().join(".codex")).unwrap();
  std::fs::write(home.path().join(".codex/auth.json"), "{}").unwrap();
  assert_eq!(
    require_auth(home.path()).unwrap(),
    home.path().join(".codex/auth.json")
  );
}

fn evidence() -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(dir.path().join("evidence/codex-home")).unwrap();
  std::fs::create_dir_all(dir.path().join("evidence/muse-home/.config/muse")).unwrap();
  dir
}

#[test]
fn a_clean_run_deletes_the_evidence_but_never_the_users_auth() {
  let dir = evidence();
  let root = dir.path().join("evidence");
  let auth = dir.path().join("auth.json");
  std::fs::write(&auth, "{}").unwrap();
  std::os::unix::fs::symlink(&auth, root.join("codex-home/auth.json")).unwrap();
  assert_eq!(settle(&root, &[], false).unwrap(), Verdict::Clean);
  assert!(!root.exists() && auth.is_file());
}

#[test]
fn kept_evidence_holds_no_auth_link() {
  let dir = evidence();
  let root = dir.path().join("evidence");
  let auth = dir.path().join("auth.json");
  std::fs::write(&auth, "{}").unwrap();
  let muse = root.join("muse-home/.config/muse/auth.json");
  std::os::unix::fs::symlink(&auth, &muse).unwrap();
  std::os::unix::fs::symlink(&auth, root.join("codex-home/auth.json")).unwrap();
  let found = vec!["muse: deny: no live deny evidence".to_owned()];
  assert_eq!(settle(&root, &found, false).unwrap(), Verdict::Findings);
  assert!(root.is_dir() && auth.is_file());
  assert!(std::fs::symlink_metadata(&muse).is_err());
  assert!(std::fs::symlink_metadata(root.join("codex-home/auth.json")).is_err());
}

#[test]
fn keep_and_failures_leave_the_evidence() {
  let dir = evidence();
  let root = dir.path().join("evidence");
  assert_eq!(settle(&root, &[], true).unwrap(), Verdict::Clean);
  assert!(root.is_dir());
  let found = vec!["claude: push review: not blocked".to_owned()];
  assert_eq!(settle(&root, &found, false).unwrap(), Verdict::Findings);
  assert!(root.is_dir());
}
