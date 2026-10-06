use std::path::Path;

use toolu_runtime::git::Repo;

use super::{abbrev_head, symbolic_target};

const OID: &str = "0123456789abcdef0123456789abcdef01234567";

/// A git dir laid out by hand, its common dir the same.
fn layout(dir: &Path, head: &str) -> Repo {
  std::fs::create_dir_all(dir.join("refs/heads")).unwrap();
  std::fs::write(dir.join("HEAD"), head).unwrap();
  Repo {
    toplevel: None,
    git_dir: dir.to_path_buf(),
    common_dir: dir.to_path_buf(),
  }
}

fn write(path: &Path, body: &str) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, body).unwrap();
}

#[test]
fn head_names_a_loose_or_packed_branch_else_reads_head() {
  let dir = tempfile::tempdir().unwrap();
  let repo = layout(dir.path(), "ref: refs/heads/feat/x\n");
  assert_eq!(abbrev_head(&repo).as_deref(), Some("HEAD"), "unborn");
  write(&dir.path().join("refs/heads/feat/x"), &format!("{OID}\n"));
  assert_eq!(abbrev_head(&repo).as_deref(), Some("feat/x"));
  std::fs::remove_file(dir.path().join("refs/heads/feat/x")).unwrap();
  let packed =
    format!("# pack-refs with: peeled fully-peeled sorted\n{OID} refs/heads/feat/x\n^{OID}\n");
  write(&dir.path().join("packed-refs"), &packed);
  assert_eq!(abbrev_head(&repo).as_deref(), Some("feat/x"), "packed only");
  write(&dir.path().join("HEAD"), &format!("{OID}\n"));
  assert_eq!(abbrev_head(&repo).as_deref(), Some("HEAD"), "detached");
}

#[test]
fn unusual_heads_and_reftable_ask_git() {
  let dir = tempfile::tempdir().unwrap();
  let repo = layout(dir.path(), "ref: refs/remotes/origin/main\n");
  assert_eq!(abbrev_head(&repo), None, "a ref outside refs/heads");
  write(&dir.path().join("HEAD"), "garbage\n");
  assert_eq!(abbrev_head(&repo), None);
  std::fs::remove_file(dir.path().join("HEAD")).unwrap();
  std::os::unix::fs::symlink("refs/heads/main", dir.path().join("HEAD")).unwrap();
  assert_eq!(abbrev_head(&repo), None, "a symlinked HEAD");
  std::fs::remove_file(dir.path().join("HEAD")).unwrap();
  write(&dir.path().join("HEAD"), "ref: refs/heads/main\n");
  std::fs::create_dir(dir.path().join("reftable")).unwrap();
  assert_eq!(abbrev_head(&repo), None);
  assert_eq!(symbolic_target(&repo, "refs/remotes/origin/HEAD"), None);
}

#[test]
fn a_short_name_git_would_lengthen_asks_git() {
  for other in [
    "refs/tags/v1",
    "refs/remotes/v1",
    "refs/remotes/v1/HEAD",
    "refs/v1",
    "v1",
  ] {
    let dir = tempfile::tempdir().unwrap();
    let repo = layout(dir.path(), "ref: refs/heads/v1\n");
    write(&dir.path().join("refs/heads/v1"), OID);
    assert_eq!(abbrev_head(&repo).as_deref(), Some("v1"));
    write(&dir.path().join(other), OID);
    assert_eq!(abbrev_head(&repo), None, "{other}");
  }
  let dir = tempfile::tempdir().unwrap();
  let repo = layout(dir.path(), "ref: refs/heads/v1\n");
  write(
    &dir.path().join("packed-refs"),
    &format!("{OID} refs/heads/v1\n{OID} refs/tags/v1\n"),
  );
  assert_eq!(abbrev_head(&repo), None, "a packed tag");
}

#[test]
fn origin_head_is_a_symbolic_ref_or_nothing() {
  let dir = tempfile::tempdir().unwrap();
  let repo = layout(dir.path(), "ref: refs/heads/main\n");
  assert_eq!(
    symbolic_target(&repo, "refs/remotes/origin/HEAD").as_deref(),
    Some("")
  );
  let file = dir.path().join("refs/remotes/origin/HEAD");
  write(&file, "ref: refs/remotes/origin/trunk\n");
  assert_eq!(
    symbolic_target(&repo, "refs/remotes/origin/HEAD").as_deref(),
    Some("refs/remotes/origin/trunk")
  );
  write(&file, OID);
  assert_eq!(
    symbolic_target(&repo, "refs/remotes/origin/HEAD").as_deref(),
    Some("")
  );
}

#[test]
fn a_symlinked_origin_head_asks_git() {
  let dir = tempfile::tempdir().unwrap();
  let repo = layout(dir.path(), "ref: refs/heads/main\n");
  std::fs::create_dir_all(dir.path().join("refs/remotes/origin")).unwrap();
  std::os::unix::fs::symlink(
    "refs/remotes/origin/develop",
    dir.path().join("refs/remotes/origin/HEAD"),
  )
  .unwrap();
  assert_eq!(symbolic_target(&repo, "refs/remotes/origin/HEAD"), None);
}
