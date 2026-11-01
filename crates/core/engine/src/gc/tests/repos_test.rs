use std::path::PathBuf;

use super::super::support::Repo;
use super::{discover, records, state_dirs_under};

#[test]
fn records_read_checkouts_and_discover_keeps_existing_git_roots_once() {
  let repo = Repo::new().unwrap();
  repo.record("a", "merged", &[]).unwrap();
  repo.record("b", "running", &["x"]).unwrap();
  std::fs::write(repo.tmp.join("epic/issues/bad.json"), "{").unwrap();
  let found = records(&[repo.tmp.join("epic"), PathBuf::from("/nowhere")]);
  assert_eq!(found.len(), 2);
  assert_eq!(found[0].key, "a");
  assert!(found[0].terminal());
  assert_eq!(found[1].leftover_files, 1);
  assert_eq!(found[1].checkout.as_deref(), Some(repo.root.as_path()));
  let linked = repo.tmp.join(".herdr/worktrees/repo/lk");
  repo.worktree(&linked, "feat/lk").unwrap();
  let roots = discover(
    &repo.env,
    &[repo.root.clone(), repo.tmp.join("missing"), linked],
    &found,
  );
  assert_eq!(
    roots,
    vec![repo.root.clone()],
    "a linked worktree names its repository"
  );
  assert_eq!(found[0].branch.as_deref(), Some("feat/a"));
  assert_eq!(
    state_dirs_under(std::slice::from_ref(&repo.tmp)),
    vec![repo.tmp.join("epic")]
  );
}
