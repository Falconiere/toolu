use super::*;

#[test]
fn recovered_worktree_requires_exact_path_commit_and_detached_state() {
  let head = "a".repeat(40);
  let listing = format!("worktree /remote/tree\0HEAD {head}\0detached\0\0");
  assert_eq!(registered(&listing, "/remote/tree", &head), Ok(true));
  assert_eq!(registered(&listing, "/remote/other", &head), Ok(false));
  assert!(registered(&listing, "/remote/tree", &"b".repeat(40)).is_err());
  assert!(
    registered(
      &listing.replace("detached", "branch refs/heads/main"),
      "/remote/tree",
      &head
    )
    .is_err()
  );
}

#[test]
fn saved_setup_reuses_the_local_git_worktree_and_rejects_mismatches() {
  let sb = crate::dispatch::sandbox::sandbox();
  let env = sb.env.clone().with("TOOLU_EPIC_MULTIPLEXER", "herdr-app");
  let mut options = sb.options(false);
  options.env = &env;
  let mut dispatcher = Dispatcher::new(&options).unwrap();
  let created = dispatcher.worktree(&sb.root, "feat/fix").unwrap();
  dispatcher.record_remote("vm", "/repo", "main").unwrap();
  let saved = dispatcher.remote().unwrap().clone();
  let worktree = field(&created, "path").unwrap();
  assert_eq!(
    field(&saved, "expectedBase").unwrap(),
    crate::dispatch::sandbox::git(&env, Path::new(worktree), &["rev-parse", "HEAD"])
  );
  let (plan, items) = (sb.dir.path().join("plan"), sb.dir.path().join("items"));
  let start = sb.start(&plan, &items);
  assert_eq!(
    dispatcher.resume_remote_worktree(&start).unwrap()["path"],
    worktree
  );
  assert!(
    dispatcher
      .setup_remote("other", "/repo", &[], "main")
      .is_err()
  );
  assert_eq!(dispatcher.remote().unwrap(), &saved);
  std::fs::write(Path::new(worktree).join("uncommitted"), "work").unwrap();
  assert_eq!(
    dispatcher.resume_remote_worktree(&start).unwrap_err().code,
    "worktree_dirty"
  );
}

#[test]
fn real_git_porcelain_identifies_a_detached_worktree() {
  let sb = crate::dispatch::sandbox::sandbox();
  let path = sb.dir.path().join("remote-tree");
  let shown = path.to_str().unwrap();
  let head = crate::dispatch::sandbox::git(&sb.env, &sb.repo, &["rev-parse", "HEAD"]);
  crate::dispatch::sandbox::git(
    &sb.env,
    &sb.repo,
    &["worktree", "add", "--detach", shown, &head],
  );
  let listing = crate::dispatch::sandbox::git(
    &sb.env,
    &sb.repo,
    &["worktree", "list", "--porcelain", "-z"],
  );
  assert_eq!(registered(&listing, shown, &head), Ok(true));
}
