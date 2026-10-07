//! Worktree bindings in real repositories and linked worktrees.

use std::path::{Path, PathBuf};

use toolu_runtime::process::{Spec, run};

use super::{ResourceBinding, bind_worktree, gitfile_target, resource_binding};

fn git(dir: &Path, args: &[&str]) {
  let mut spec = Spec::new(["git"]);
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  spec.cwd = Some(dir.to_path_buf());
  let output = run(&spec).unwrap();
  assert_eq!(output.exit_code, 0, "git {args:?}: {}", output.stderr);
}

fn repo() -> (tempfile::TempDir, PathBuf) {
  let dir = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(dir.path()).unwrap().join("main");
  std::fs::create_dir(&root).unwrap();
  git(&root, &["init", "-q", "-b", "main"]);
  git(
    &root,
    &[
      "-c",
      "user.name=t",
      "-c",
      "user.email=t@t",
      "commit",
      "-q",
      "--allow-empty",
      "-m",
      "init",
    ],
  );
  (dir, root)
}

#[test]
fn a_bound_worktree_reads_its_binding_from_any_subdirectory() {
  let (_dir, root) = repo();
  assert_eq!(resource_binding(&root), Ok(None));
  bind_worktree(
    Path::new("/srv/resources"),
    &root,
    "issue-421",
    "/epics/402",
  )
  .unwrap();
  let written = std::fs::read_to_string(root.join(".git/toolu-resource.json")).unwrap();
  let expected = format!(
    "{{\n  \"version\": 1,\n  \"root\": \"/srv/resources\",\n  \"key\": \"issue-421\",\n  \"stateDir\": \"/epics/402\",\n  \"worktree\": \"{}\"\n}}\n",
    root.display()
  );
  assert_eq!(written, expected);
  std::fs::create_dir_all(root.join("a/b")).unwrap();
  assert_eq!(
    resource_binding(&root.join("a/b")),
    Ok(Some(ResourceBinding {
      root: PathBuf::from("/srv/resources"),
      key: "issue-421".to_owned(),
      state_dir: "/epics/402".to_owned(),
      worktree: root.clone(),
    }))
  );
}

#[test]
fn a_linked_worktree_keeps_its_binding_in_its_own_git_dir() {
  let (dir, root) = repo();
  let linked = std::fs::canonicalize(dir.path()).unwrap().join("linked");
  git(
    &root,
    &[
      "worktree",
      "add",
      "-q",
      "-b",
      "feat/x",
      linked.to_str().unwrap(),
    ],
  );
  bind_worktree(Path::new("/r"), &linked, "k", "s").unwrap();
  assert!(
    root
      .join(".git/worktrees/linked/toolu-resource.json")
      .exists()
  );
  assert_eq!(resource_binding(&root), Ok(None));
  let bound = resource_binding(&linked).unwrap().unwrap();
  assert_eq!(bound.worktree, linked);
}

#[test]
fn an_invalid_binding_or_marker_is_an_error() {
  let (dir, root) = repo();
  let file = root.join(".git/toolu-resource.json");
  for body in [
    r#"{"version":2,"root":"/r","key":"k","stateDir":"s","worktree":"WT"}"#,
    r#"{"version":1,"root":"relative","key":"k","stateDir":"s","worktree":"WT"}"#,
    r#"{"version":1,"root":"/r","key":"k","stateDir":"s","worktree":"/elsewhere"}"#,
    r#"{"version":1,"root":"/r","stateDir":"s","worktree":"WT"}"#,
  ] {
    std::fs::write(&file, body.replace("WT", &root.display().to_string())).unwrap();
    assert_eq!(
      resource_binding(&root),
      Err("invalid worktree resource binding".to_owned()),
      "{body}"
    );
  }
  std::fs::write(&file, "{").unwrap();
  assert!(resource_binding(&root).is_err());
  let outside = std::fs::canonicalize(dir.path()).unwrap().join("bad");
  std::fs::create_dir(&outside).unwrap();
  std::fs::write(outside.join(".git"), "not a marker\n").unwrap();
  let marker = format!(
    "invalid git worktree marker: {}",
    outside.join(".git").display()
  );
  assert_eq!(resource_binding(&outside), Err(marker));
  assert!(resource_binding(&outside.join("missing")).is_err());
}

#[test]
fn binding_outside_a_repository_is_refused() {
  let dir = tempfile::tempdir().unwrap();
  let result = bind_worktree(Path::new("/r"), dir.path(), "k", "s");
  assert_eq!(
    result,
    Err("cannot bind resources outside a Git worktree".to_owned())
  );
}

#[test]
fn a_gitfile_names_its_target_on_the_first_line() {
  assert_eq!(gitfile_target("gitdir: /a/b\n"), Some("/a/b"));
  assert_eq!(gitfile_target("gitdir: rel/x \r\n\n "), Some("rel/x "));
  assert_eq!(gitfile_target("gitdir: /a\nmore"), None);
  assert_eq!(gitfile_target("gitdir: \n"), None);
  assert_eq!(gitfile_target("other: /a\n"), None);
}
