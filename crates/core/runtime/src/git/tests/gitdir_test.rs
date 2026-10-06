use std::path::Path;

use super::{common_dir_of, is_git_dir, is_object_id, read_gitfile};

/// A minimal git dir by hand: `HEAD`, `objects/`, `refs/`.
fn bare_layout(dir: &Path, head: &str) {
  std::fs::create_dir_all(dir.join("objects")).unwrap();
  std::fs::create_dir_all(dir.join("refs")).unwrap();
  std::fs::write(dir.join("HEAD"), head).unwrap();
}

#[test]
fn object_ids_are_forty_or_sixty_four_hex_digits() {
  assert!(is_object_id(&"a".repeat(40)));
  assert!(is_object_id(&"0F".repeat(32)));
  assert!(!is_object_id(&"a".repeat(39)));
  assert!(!is_object_id(&format!("{}g", "a".repeat(39))));
  assert!(!is_object_id(""));
}

#[test]
fn a_git_dir_needs_a_valid_head_objects_and_refs() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path();
  assert!(!is_git_dir(path));
  bare_layout(path, "ref: refs/heads/main\n");
  assert!(is_git_dir(path));
  std::fs::write(path.join("HEAD"), format!("{}\n", "b".repeat(40))).unwrap();
  assert!(is_git_dir(path), "a detached HEAD is valid");
  std::fs::write(path.join("HEAD"), "ref:refs/heads/x").unwrap();
  assert!(is_git_dir(path), "no space after ref: is valid");
  std::fs::write(path.join("HEAD"), "ref: heads/main\n").unwrap();
  assert!(!is_git_dir(path), "a ref outside refs/ is not");
  std::fs::write(path.join("HEAD"), "garbage\n").unwrap();
  assert!(!is_git_dir(path));
  std::fs::remove_file(path.join("HEAD")).unwrap();
  std::os::unix::fs::symlink("refs/heads/main", path.join("HEAD")).unwrap();
  assert!(is_git_dir(path), "a symlinked HEAD into refs/ is valid");
  std::fs::remove_dir(path.join("objects")).unwrap();
  assert!(!is_git_dir(path), "objects/ is required");
}

#[test]
fn a_commondir_file_names_the_common_dir_relative_to_the_git_dir() {
  let root = tempfile::tempdir().unwrap();
  let common = root.path().join("main.git");
  let admin = common.join("worktrees").join("wt");
  bare_layout(&common, "ref: refs/heads/main\n");
  std::fs::create_dir_all(&admin).unwrap();
  std::fs::write(admin.join("HEAD"), "ref: refs/heads/wt\n").unwrap();
  std::fs::write(admin.join("commondir"), "../..\n").unwrap();
  let real = std::fs::canonicalize(&common).unwrap();
  assert_eq!(common_dir_of(&admin), real);
  assert!(
    is_git_dir(&admin),
    "objects and refs come from the common dir"
  );
  assert_eq!(common_dir_of(&common), common);
}

#[test]
fn a_gitfile_names_a_git_dir_or_is_invalid() {
  let root = tempfile::tempdir().unwrap();
  let target = root.path().join("store.git");
  bare_layout(&target, "ref: refs/heads/main\n");
  let work = root.path().join("work");
  std::fs::create_dir(&work).unwrap();
  let file = work.join(".git");
  std::fs::write(&file, "gitdir: ../store.git\n").unwrap();
  let real = std::fs::canonicalize(&target).unwrap();
  assert_eq!(read_gitfile(&file), Some(real.clone()));
  std::fs::write(&file, format!("gitdir: {}\r\n", target.display())).unwrap();
  assert_eq!(read_gitfile(&file), Some(real));
  for bad in [
    "gitdir:../store.git\n",
    "gitdir: \n",
    "nonsense\n",
    "gitdir: ../missing\n",
  ] {
    std::fs::write(&file, bad).unwrap();
    assert_eq!(read_gitfile(&file), None, "{bad:?}");
  }
  assert_eq!(read_gitfile(&work.join("absent")), None);
}

#[test]
fn gitfile_and_commondir_paths_are_bytes_not_utf8() {
  use std::os::unix::ffi::OsStrExt as _;
  let root = tempfile::tempdir().unwrap();
  let name = std::ffi::OsStr::from_bytes(b"st\xe9re.git");
  let target = root.path().join(name);
  bare_layout(&target, "ref: refs/heads/main\n");
  let file = root.path().join(".git");
  let mut body = b"gitdir: ".to_vec();
  body.extend_from_slice(name.as_bytes());
  body.extend_from_slice(b"\r\n");
  std::fs::write(&file, &body).unwrap();
  assert_eq!(
    read_gitfile(&file),
    Some(std::fs::canonicalize(&target).unwrap())
  );
  let admin = target.join("worktrees/w");
  std::fs::create_dir_all(&admin).unwrap();
  std::fs::write(admin.join("HEAD"), "ref: refs/heads/w\n").unwrap();
  let mut up = b"../../".to_vec();
  up.push(b'\n');
  std::fs::write(admin.join("commondir"), up).unwrap();
  assert_eq!(
    common_dir_of(&admin),
    std::fs::canonicalize(&target).unwrap()
  );
}
