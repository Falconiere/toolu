use crate::finish_rig::layout;

#[test]
fn remote_registration_checks_real_git_path_and_branch() {
  let l = layout();
  let output = std::process::Command::new("git")
    .args([
      "-C",
      l.main.to_str().expect("path"),
      "worktree",
      "list",
      "--porcelain",
      "-z",
    ])
    .output()
    .expect("git");
  assert!(output.status.success());
  let listing = std::str::from_utf8(&output.stdout).expect("utf8");
  let tree = l.wt.to_str().expect("path");
  assert_eq!(super::registered(listing, tree, "feat/1-x"), Ok(true));
  assert!(super::registered(listing, tree, "foreign").is_err());
  assert_eq!(
    super::registered(listing, "/missing", "feat/1-x"),
    Ok(false)
  );
}
