//! `toolu epic finish` from the real binary against a real checkout.

#[path = "helpers/epic.rs"]
mod epic;
#[path = "helpers/epic_doc.rs"]
mod epic_doc;

use std::path::Path;
use std::process::Command;

use epic::{Home, Res, err};
use epic_doc::doc;

fn git(cwd: &Path, args: &[&str]) -> Res<()> {
  let status = Command::new("git")
    .args(args)
    .current_dir(cwd)
    .env("GIT_AUTHOR_NAME", "t")
    .env("GIT_AUTHOR_EMAIL", "t@t")
    .env("GIT_COMMITTER_NAME", "t")
    .env("GIT_COMMITTER_EMAIL", "t@t")
    .output()?;
  if status.status.success() {
    return Ok(());
  }
  Err(format!("git {args:?}: {}", String::from_utf8_lossy(&status.stderr)).into())
}

/// origin <- main, a linked worktree on feat/1-x, and the record of k-1.
fn layout(home: &Home) -> Res<(std::path::PathBuf, std::path::PathBuf)> {
  let root = std::fs::canonicalize(home.dir.path())?;
  let (origin, main, wt) = (root.join("origin.git"), root.join("main"), root.join("wt"));
  git(
    &root,
    &[
      "init",
      "-q",
      "--bare",
      "-b",
      "main",
      &origin.display().to_string(),
    ],
  )?;
  git(
    &root,
    &[
      "clone",
      "-q",
      &origin.display().to_string(),
      &main.display().to_string(),
    ],
  )?;
  std::fs::write(main.join("a.txt"), "one\n")?;
  git(&main, &["add", "a.txt"])?;
  git(&main, &["commit", "-qm", "init"])?;
  git(&main, &["push", "-q", "origin", "main"])?;
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
  )?;
  let record = serde_json::json!({"key": "k-1", "repo": "o/r", "branch": "feat/1-x", "checkout": main, "worktree": wt, "stage": "running"});
  let path = root.join("state/issues/k-1.json");
  std::fs::create_dir_all(path.parent().ok_or("no parent")?)?;
  std::fs::write(&path, record.to_string())?;
  Ok((root.join("state"), wt))
}

#[test]
fn abandon_snapshots_dirty_work_and_exits_1_while_cleanup_is_incomplete() {
  let home = Home::new().unwrap();
  let (state, wt) = layout(&home).unwrap();
  std::fs::write(wt.join("a.txt"), "two\n").expect("dirty");
  let output = home
    .run(&[
      "epic",
      "finish",
      &state.display().to_string(),
      "k-1",
      "--abandon",
    ])
    .unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert!(
    err(&output).contains("WARN: no workspace_id on record k-1"),
    "{}",
    err(&output)
  );
  let result = doc(&output).unwrap();
  assert_eq!(
    (result["stage"].as_str(), result["wip_ref"].as_str()),
    (Some("cleanup-incomplete"), Some("refs/epic-wip/k-1"))
  );
}

#[test]
fn refusals_leave_the_record_and_usage_errors_exit_64() {
  let home = Home::new().unwrap();
  let (state, _) = layout(&home).unwrap();
  let state = state.display().to_string();
  let before =
    std::fs::read_to_string(home.dir.path().join("state/issues/k-1.json")).expect("record");
  let refused = home.run(&["epic", "finish", &state, "k-1"]).unwrap();
  assert_eq!(refused.status.code(), Some(1));
  assert!(err(&refused).contains("no PR recorded for k-1; use --abandon"));
  assert_eq!(
    std::fs::read_to_string(home.dir.path().join("state/issues/k-1.json")).expect("record"),
    before
  );
  let missing = home
    .run(&["epic", "finish", &state, "k-9", "--abandon"])
    .unwrap();
  assert!(err(&missing).contains("invalid or missing issue record"));
  for args in [
    vec!["epic", "finish", &state],
    vec!["epic", "finish", &state, "k-1", "--force"],
  ] {
    assert_eq!(home.run(&args).unwrap().status.code(), Some(64), "{args:?}");
  }
}
