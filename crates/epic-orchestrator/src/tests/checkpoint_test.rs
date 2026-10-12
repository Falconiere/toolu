use std::path::{Path, PathBuf};

use super::{snapshot, snapshot_active};
use crate::support::git;

/// origin (bare) <- main clone, plus a linked worktree on a feature branch.
fn setup(root: &Path) -> (PathBuf, PathBuf) {
  let origin = root.join("origin.git");
  let main = root.join("main");
  let files = [
    ("a.txt", "one\n"),
    ("delete.txt", "remove me\n"),
    (".gitignore", "node_modules/\n"),
  ];
  crate::support::origin_with_clone(&origin, &main, &files);
  let wt = root.join("wt");
  git(
    &main,
    &[
      "worktree",
      "add",
      "-q",
      "-b",
      "feat/1-x",
      &wt.display().to_string(),
      "origin/main",
    ],
  );
  (main, wt)
}

fn path(wt: &Path) -> String {
  wt.display().to_string()
}

fn retained(wt: &Path) -> PathBuf {
  let common = git(
    wt,
    &["rev-parse", "--path-format=absolute", "--git-common-dir"],
  );
  let root = Path::new(&common).join("toolu").join("checkpoint");
  let entry = std::fs::read_dir(&root)
    .expect("root")
    .filter_map(Result::ok)
    .find(|entry| entry.path().is_dir());
  entry.expect("retained index").path()
}

fn traced(trace: &Path, suffix: &str) -> usize {
  let text = std::fs::read_to_string(trace).unwrap_or_default();
  text
    .lines()
    .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
    .filter(|event| event["event"] == "start")
    .filter_map(|event| {
      let argv = event["argv"]
        .as_array()?
        .iter()
        .filter_map(|part| part.as_str())
        .collect::<Vec<_>>();
      Some(argv.join(" "))
    })
    .filter(|command| {
      command.ends_with(suffix) || (suffix == " commit-tree" && command.contains(" commit-tree "))
    })
    .count()
}

#[test]
fn a_clean_pushed_worktree_has_nothing_at_risk() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_, wt) = setup(tmp.path());
  let shot = snapshot(&path(&wt), "k-1", None);
  assert_eq!(shot.skipped.as_deref(), Some("nothing at risk"));
  assert!(!shot.changed && !shot.failed());
  let missing = snapshot(&path(&tmp.path().join("gone")), "k-1", None);
  assert_eq!(missing.skipped.as_deref(), Some("no worktree"));
}

#[test]
fn tracked_staged_and_untracked_work_is_captured_without_touching_the_index() {
  let tmp = tempfile::tempdir().expect("temp");
  let (main, wt) = setup(tmp.path());
  std::fs::write(wt.join("a.txt"), "two\n").expect("a");
  std::fs::write(wt.join("new.txt"), "untracked\n").expect("new");
  std::fs::write(wt.join("staged.txt"), "staged\n").expect("staged");
  git(&wt, &["add", "staged.txt"]);
  std::fs::remove_file(wt.join("delete.txt")).expect("delete");
  let before = git(&wt, &["status", "--porcelain"]);
  let index = wt.join(git(&wt, &["rev-parse", "--git-path", "index"]));
  let index_before = std::fs::read(&index).expect("index");
  let shot = snapshot(&path(&wt), "k-1", None);
  assert!(shot.changed && shot.dirty, "{shot:?}");
  assert_eq!(git(&wt, &["status", "--porcelain"]), before);
  assert_eq!(std::fs::read(&index).expect("index"), index_before);
  git(&main, &["worktree", "remove", "--force", &path(&wt)]);
  assert_eq!(git(&main, &["show", "refs/epic-wip/k-1:a.txt"]), "two");
  assert_eq!(
    git(&main, &["show", "refs/epic-wip/k-1:new.txt"]),
    "untracked"
  );
  assert_eq!(
    git(&main, &["show", "refs/epic-wip/k-1:staged.txt"]),
    "staged"
  );
  let deleted = std::process::Command::new("git")
    .args([
      "-C",
      &path(&main),
      "cat-file",
      "-e",
      "refs/epic-wip/k-1:delete.txt",
    ])
    .output()
    .expect("git");
  assert!(!deleted.status.success());
}

#[test]
fn an_unchanged_tree_adds_no_reflog_entry_and_new_work_does() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_, wt) = setup(tmp.path());
  std::fs::write(wt.join("a.txt"), "two\n").expect("a");
  assert!(snapshot(&path(&wt), "k-2", None).changed);
  assert!(!snapshot(&path(&wt), "k-2", None).changed);
  std::fs::write(wt.join("a.txt"), "three\n").expect("a");
  assert!(snapshot(&path(&wt), "k-2", None).changed);
  let reflog = git(&wt, &["reflog", "show", "--format=%H", "refs/epic-wip/k-2"]);
  assert_eq!(reflog.lines().count(), 2);
}

#[test]
fn unpushed_commits_on_a_clean_tree_are_pinned() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_, wt) = setup(tmp.path());
  std::fs::write(wt.join("a.txt"), "committed\n").expect("a");
  git(&wt, &["commit", "-qam", "wip"]);
  let shot = snapshot(&path(&wt), "k-3", None);
  assert_eq!(shot.unpushed, 1);
  assert_eq!(shot.sha, Some(git(&wt, &["rev-parse", "HEAD"])));
  assert!(shot.changed && !shot.dirty);
  assert!(!snapshot(&path(&wt), "k-3", None).changed);
}

#[test]
fn the_retained_index_avoids_redundant_initialization_and_commits() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_, wt) = setup(tmp.path());
  let trace = tmp.path().join("trace.json");
  std::fs::write(wt.join("a.txt"), "dirty\n").expect("a");
  assert!(snapshot(&path(&wt), "k-trace", Some(&trace)).changed);
  assert!(!snapshot(&path(&wt), "k-trace", Some(&trace)).changed);
  assert_eq!(traced(&trace, " read-tree HEAD"), 1);
  assert_eq!(traced(&trace, " commit-tree"), 1);
  assert_eq!(traced(&trace, " add -A"), 2);
  assert_eq!(traced(&trace, " write-tree"), 2);
}

#[test]
fn a_new_head_resets_the_retained_index_and_the_ancestry() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_, wt) = setup(tmp.path());
  let trace = tmp.path().join("head-trace.json");
  std::fs::write(wt.join("a.txt"), "first\n").expect("a");
  assert!(snapshot(&path(&wt), "k-head", Some(&trace)).changed);
  git(&wt, &["add", "a.txt"]);
  git(&wt, &["commit", "-qm", "advance head"]);
  let head = git(&wt, &["rev-parse", "HEAD"]);
  std::fs::write(wt.join("new.txt"), "after head\n").expect("new");
  assert!(snapshot(&path(&wt), "k-head", Some(&trace)).changed);
  assert_eq!(git(&wt, &["rev-parse", "refs/epic-wip/k-head^"]), head);
  assert_eq!(
    git(&wt, &["show", "refs/epic-wip/k-head:new.txt"]),
    "after head"
  );
  assert_eq!(traced(&trace, " read-tree HEAD"), 2);
}

#[test]
fn a_corrupt_retained_index_is_reset_from_the_same_head() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_, wt) = setup(tmp.path());
  std::fs::write(wt.join("a.txt"), "dirty\n").expect("a");
  assert!(snapshot(&path(&wt), "k-corrupt", None).changed);
  std::fs::write(retained(&wt).join("index"), "corrupt\n").expect("corrupt");
  let shot = snapshot(&path(&wt), "k-corrupt", None);
  assert!(!shot.changed);
  assert_eq!(shot.skipped, None);
}

#[test]
fn a_locked_retained_index_moves_to_a_fresh_private_generation() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_, wt) = setup(tmp.path());
  std::fs::write(wt.join("a.txt"), "first\n").expect("a");
  assert!(snapshot(&path(&wt), "k-locked", None).changed);
  let directory = retained(&wt);
  let abandoned = directory.join("index.lock");
  std::fs::write(&abandoned, "orphan writer").expect("lock");
  std::fs::write(wt.join("a.txt"), "after crash\n").expect("a");
  let shot = snapshot(&path(&wt), "k-locked", None);
  assert!(shot.changed && shot.skipped.is_none(), "{shot:?}");
  assert_eq!(
    git(&wt, &["show", "refs/epic-wip/k-locked:a.txt"]),
    "after crash"
  );
  let meta: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(directory.join("meta.json")).expect("meta"))
      .expect("json");
  let index = meta["index"].as_str().expect("index");
  assert!(
    regex::Regex::new(r"^index-[0-9a-f-]{36}$")
      .expect("re")
      .is_match(index),
    "{index}"
  );
  assert!(directory.join(index).exists() && abandoned.exists());
}

#[test]
fn a_missing_retained_index_is_rebuilt_before_staging_ignored_tracked_files() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_, wt) = setup(tmp.path());
  std::fs::write(wt.join(".gitignore"), "node_modules/\na.txt\n").expect("ignore");
  git(&wt, &["add", ".gitignore"]);
  git(&wt, &["commit", "-qm", "ignore tracked file"]);
  std::fs::write(wt.join("a.txt"), "first snapshot\n").expect("a");
  assert!(snapshot(&path(&wt), "k-missing-index", None).changed);
  std::fs::remove_file(retained(&wt).join("index")).expect("remove");
  std::fs::write(wt.join("a.txt"), "after index deletion\n").expect("a");
  assert!(snapshot(&path(&wt), "k-missing-index", None).changed);
  assert_eq!(
    git(&wt, &["show", "refs/epic-wip/k-missing-index:a.txt"]),
    "after index deletion"
  );
}

#[test]
fn concurrent_attempts_serialize_on_one_retained_index() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_, wt) = setup(tmp.path());
  std::fs::write(wt.join("a.txt"), "dirty\n").expect("a");
  let worktree = path(&wt);
  let shots: Vec<_> = std::thread::scope(|scope| {
    let handles: Vec<_> = (0..2)
      .map(|_| scope.spawn(|| snapshot(&worktree, "k-lock", None)))
      .collect();
    handles
      .into_iter()
      .map(|handle| handle.join().expect("join"))
      .collect()
  });
  assert_eq!(
    shots.iter().filter(|shot| shot.changed).count(),
    1,
    "{shots:?}"
  );
  assert!(shots.iter().all(|shot| shot.changed
    || shot.skipped.as_deref() == Some("checkpoint busy")
    || shot.sha.is_some()));
  let reflog = git(
    &wt,
    &["reflog", "show", "--format=%H", "refs/epic-wip/k-lock"],
  );
  assert_eq!(reflog.lines().count(), 1);
}

#[test]
fn only_running_issues_are_visited_unless_a_key_is_named() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_, wt) = setup(tmp.path());
  std::fs::write(wt.join("a.txt"), "dirty\n").expect("a");
  let state = tmp.path().join("state");
  let record = serde_json::json!({"stage": "merged", "worktree": path(&wt)});
  crate::disk::write_value(&state.join("issues").join("k-4.json"), &record).expect("record");
  assert_eq!(snapshot_active(&state, None, None), Vec::new());
  let named = snapshot_active(&state, Some("k-4"), None);
  assert!(named.first().is_some_and(|shot| shot.changed), "{named:?}");
  assert_eq!(
    snapshot_active(&tmp.path().join("none"), None, None),
    Vec::new()
  );
  let json = serde_json::to_value(&named[0]).expect("json");
  assert_eq!(json["ref"], "refs/epic-wip/k-4");
  assert!(json.get("skipped").is_none());
}

#[test]
fn a_lock_that_cannot_be_taken_fails_the_snapshot_instead_of_reading_busy() {
  use std::os::unix::fs::PermissionsExt;
  let tmp = tempfile::tempdir().expect("temp");
  let (_, wt) = setup(tmp.path());
  std::fs::write(wt.join("a.txt"), "dirty\n").expect("dirty");
  let first = snapshot(&path(&wt), "k-1", None);
  assert!(first.changed, "{first:?}");
  let root = retained(&wt)
    .parent()
    .expect("checkpoint root")
    .to_path_buf();
  std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o500)).expect("lock out");
  std::fs::write(wt.join("a.txt"), "dirtier\n").expect("dirtier");
  let shot = snapshot(&path(&wt), "k-1", None);
  std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).expect("restore");
  assert_eq!(shot.skipped.as_deref(), Some("checkpoint lock failed"));
  assert!(shot.failed());
}

#[test]
fn a_checkpoint_directory_that_cannot_be_made_fails_with_its_own_reason() {
  let tmp = tempfile::tempdir().expect("temp");
  let (main, wt) = setup(tmp.path());
  std::fs::write(wt.join("a.txt"), "dirty\n").expect("dirty");
  // A file where the checkpoint tree's `toolu` directory would go.
  std::fs::write(main.join(".git").join("toolu"), "in the way\n").expect("blocker");
  let shot = snapshot(&path(&wt), "k-1", None);
  assert_eq!(shot.skipped.as_deref(), Some("checkpoint directory failed"));
  assert!(shot.failed());
}
