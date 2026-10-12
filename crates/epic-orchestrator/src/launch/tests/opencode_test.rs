use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;

use super::{
  EXCLUDE_MARKER, OPENCODE_EXCLUDE, check_opencode, exclude_for_worker, exclude_opencode_state,
};
use crate::support::git;

fn env(path: &str) -> Env {
  Env::from_pairs([("PATH", path), ("HOME", "/tmp")])
}

fn repo(root: &Path) -> (PathBuf, PathBuf) {
  let main = root.join("main");
  crate::support::repo_with_origin(&main, "https://github.com/o/r.git");
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
    ],
  );
  (main, wt)
}

#[test]
fn excluded_state_stays_out_of_status_and_snapshots() {
  let tmp = tempfile::tempdir().expect("temp");
  let (main, wt) = repo(tmp.path());
  let _gone = std::fs::remove_dir_all(main.join(".git/info"));
  for (path, body) in [
    (".opencode/toolu/state/x", "registry\n"),
    (".opencode/tmp/y", "telemetry\n"),
    (".opencode/toolu/plugins.json", "{}\n"),
  ] {
    let file = wt.join(path);
    std::fs::create_dir_all(file.parent().expect("parent")).expect("dir");
    std::fs::write(file, body).expect("file");
  }
  let path = exclude_opencode_state(&wt, &env("/usr/bin:/bin")).expect("exclude");
  assert_eq!(
    exclude_opencode_state(&main, &env("/usr/bin:/bin")).as_ref(),
    Ok(&path)
  );
  assert_eq!(
    std::fs::canonicalize(&path).ok(),
    std::fs::canonicalize(main.join(".git/info/exclude")).ok()
  );
  let expected = format!(
    "{}\n",
    [EXCLUDE_MARKER]
      .into_iter()
      .chain(OPENCODE_EXCLUDE)
      .collect::<Vec<_>>()
      .join("\n")
  );
  assert_eq!(std::fs::read_to_string(&path).expect("exclude"), expected);
  assert_eq!(
    git(&wt, &["status", "--porcelain", "--untracked-files=all"]),
    "?? .opencode/toolu/plugins.json"
  );
  let snap = crate::checkpoint::snapshot(&wt.display().to_string(), "k-1", None);
  assert!(snap.changed, "{snap:?}");
  let tree = git(&wt, &["ls-tree", "-r", "--name-only", "refs/epic-wip/k-1"]);
  assert!(
    tree
      .lines()
      .any(|path| path == ".opencode/toolu/plugins.json"),
    "{tree}"
  );
  assert!(!tree.contains(".opencode/toolu/state"), "{tree}");
  assert!(!tree.contains(".opencode/tmp"), "{tree}");
}

#[test]
fn the_block_is_appended_once_and_trailing_spaces_count() {
  let tmp = tempfile::tempdir().expect("temp");
  let (main, wt) = repo(tmp.path());
  let path = main.join(".git/info/exclude");
  std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
  std::fs::write(
    &path,
    format!(
      "node_modules\n{EXCLUDE_MARKER}\n{}\ncoverage",
      OPENCODE_EXCLUDE[0]
    ),
  )
  .expect("seed");
  exclude_opencode_state(&wt, &env("/usr/bin:/bin")).expect("first");
  exclude_opencode_state(&main, &env("/usr/bin:/bin")).expect("second");
  let want = format!(
    "node_modules\n{EXCLUDE_MARKER}\n{}\ncoverage\n{}\n",
    OPENCODE_EXCLUDE[0], OPENCODE_EXCLUDE[1]
  );
  assert_eq!(std::fs::read_to_string(&path).expect("exclude"), want);
  let spaced = format!(
    "{EXCLUDE_MARKER}  \n{} \n{}\t\n",
    OPENCODE_EXCLUDE[0], OPENCODE_EXCLUDE[1]
  );
  std::fs::write(&path, &spaced).expect("spaced");
  exclude_opencode_state(&wt, &env("/usr/bin:/bin")).expect("third");
  assert_eq!(std::fs::read_to_string(&path).expect("exclude"), spaced);
  let outside = exclude_opencode_state(tmp.path(), &env("/usr/bin:/bin")).expect_err("outside");
  assert!(outside.message.contains("rev-parse"), "{}", outside.message);
  let dry = exclude_for_worker(&tmp.path().join("none"), true, &env("")).expect("dry");
  assert!(dry.starts_with("# opencode: exclude /.opencode/toolu/state/ /.opencode/tmp/ in "));
}

/// A `PATH` whose `opencode --version` prints `version`.
fn fake_opencode(dir: &Path, version: &str) -> Env {
  let bin = dir.join("bin");
  std::fs::create_dir_all(&bin).expect("bin");
  let script = bin.join("opencode");
  std::fs::write(&script, format!("#!/bin/sh\necho {version}\n")).expect("script");
  std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
  env(&format!("{}:/usr/bin:/bin", bin.display()))
}

#[test]
fn a_missing_or_wrong_opencode_stops_before_side_effects() {
  let tmp = tempfile::tempdir().expect("temp");
  assert!(check_opencode(Some("probe/scripted"), true, &env("")).is_ok());
  let model = check_opencode(Some("sonnet"), true, &env("")).expect_err("model");
  assert_eq!(
    model.message,
    "OpenCode model must be provider/model, got sonnet"
  );
  let empty = env(&tmp.path().join("empty").display().to_string());
  let missing = check_opencode(None, false, &empty).expect_err("missing");
  assert!(
    missing
      .message
      .starts_with("opencode is not runnable here ("),
    "{}",
    missing.message
  );
  assert!(
    missing
      .message
      .ends_with("Put a 1.x opencode first on PATH, or route this issue to another host.")
  );
  let old = check_opencode(None, false, &fake_opencode(tmp.path(), "0.15.4")).expect_err("old");
  assert!(
    old
      .message
      .starts_with("opencode --version reports \"0.15.4\""),
    "{}",
    old.message
  );
  assert!(check_opencode(None, false, &fake_opencode(tmp.path(), "1.2.3")).is_ok());
}
