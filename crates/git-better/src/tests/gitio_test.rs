//! A commit's file changes from real git: a root commit, a rename with an
//! edit, a binary file and a merge against its first parent.

use toolu_runtime::env::Env;

use super::{FileChange, changes, git};
use crate::git_support::git as run;

fn env() -> Env {
  Env::from_pairs([("PATH", std::env::var("PATH").unwrap_or_default())])
}

fn change(path: &str, old: Option<&str>, status: &str, counts: Option<(u64, u64)>) -> FileChange {
  FileChange {
    path: path.to_owned(),
    old_path: old.map(str::to_owned),
    status: status.to_owned(),
    added: counts.map(|(added, _)| added),
    deleted: counts.map(|(_, deleted)| deleted),
  }
}

#[test]
fn changes_cover_root_rename_binary_and_merge_commits() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  run(root, &["init", "-q", "-b", "main"]);
  std::fs::create_dir_all(root.join("crates/a")).unwrap();
  std::fs::write(root.join("crates/a/f.rs"), "one\ntwo\nthree\nfour\n").unwrap();
  std::fs::write(root.join("README.md"), "x\n").unwrap();
  run(root, &["add", "-A"]);
  run(root, &["commit", "-q", "-m", "root"]);
  let env = env();
  assert_eq!(
    changes(&env, root, "HEAD").unwrap(),
    [
      change("README.md", None, "A", Some((1, 0))),
      change("crates/a/f.rs", None, "A", Some((4, 0))),
    ]
  );
  run(root, &["mv", "crates/a/f.rs", "crates/a/g.rs"]);
  std::fs::write(root.join("crates/a/g.rs"), "one\ntwo\nthree\nfive\n").unwrap();
  std::fs::write(root.join("logo.bin"), [0_u8, 159, 146, 150]).unwrap();
  run(root, &["add", "-A"]);
  run(root, &["commit", "-q", "-m", "second"]);
  assert_eq!(
    changes(&env, root, "HEAD").unwrap(),
    [
      change("crates/a/g.rs", Some("crates/a/f.rs"), "R", Some((1, 1))),
      change("logo.bin", None, "A", None),
    ]
  );
  run(root, &["switch", "-q", "-c", "feat/x"]);
  std::fs::write(root.join("b.txt"), "b\n").unwrap();
  run(root, &["add", "-A"]);
  run(root, &["commit", "-q", "-m", "b"]);
  run(root, &["switch", "-q", "main"]);
  std::fs::write(root.join("m.txt"), "m\n").unwrap();
  run(root, &["add", "-A"]);
  run(root, &["commit", "-q", "-m", "m"]);
  run(root, &["merge", "-q", "--no-edit", "feat/x"]);
  assert_eq!(
    changes(&env, root, "HEAD").unwrap(),
    [change("b.txt", None, "A", Some((1, 0)))]
  );
}

#[test]
fn a_failing_git_command_is_an_error_naming_it() {
  let dir = tempfile::tempdir().unwrap();
  run(dir.path(), &["init", "-q"]);
  let error = git(&env(), dir.path(), &["rev-parse", "nope"], &[]).unwrap_err();
  assert!(error.starts_with("git rev-parse nope failed"), "{error}");
  assert!(changes(&env(), dir.path(), "HEAD").is_err());
}
