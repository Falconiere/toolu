//! Reading a commit message before the command runs: what is read, and every
//! case that is reported instead of guessed.

use std::path::Path;

use toolu_shell::git::{commit_args, git_invocation};

use super::{Line, MAX_MESSAGE_BYTES, message};

/// The message of the first commit in `source`, read in `dir`.
fn read(source: &str, dir: &Path) -> Result<Option<String>, String> {
  let analysis = toolu_shell::analyze(source);
  let line = Line::of(&analysis);
  let args = analysis
    .commands
    .iter()
    .filter_map(git_invocation)
    .find_map(|git| commit_args(&git))
    .unwrap();
  message(&args, dir, &line)
}

#[test]
fn paragraphs_files_stdin_and_trailers_join_as_git_joins_them() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("m.txt"), "fix(x): y\n").unwrap();
  assert_eq!(
    read(
      "git commit -m 'fix(x): y' -m 'Body.' --trailer 'Intent=why' --trailer 'Risk: low'",
      dir.path()
    ),
    Ok(Some(
      "fix(x): y\n\nBody.\n\nIntent: why\nRisk: low".to_owned()
    ))
  );
  assert_eq!(
    read("git commit -F m.txt --trailer Intent:why", dir.path()),
    Ok(Some("fix(x): y\n\nIntent: why".to_owned()))
  );
  assert_eq!(
    read("git commit -F - <<'EOF'\nfix(x): y\nEOF", dir.path()),
    Ok(Some("fix(x): y".to_owned()))
  );
  assert_eq!(read("git commit --amend --no-edit", dir.path()), Ok(None));
}

#[test]
fn what_only_exists_at_run_time_is_reported() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("m.txt"), "fix(x): stale\n").unwrap();
  std::fs::create_dir(dir.path().join("sub")).unwrap();
  let big = "x".repeat(usize::try_from(MAX_MESSAGE_BYTES).unwrap() + 1);
  std::fs::write(dir.path().join("big.txt"), big).unwrap();
  let cases = [
    ("git commit -m \"$M\"", "its `-m` value expands at run time"),
    ("git commit -F \"$F\"", "its `-F` path expands at run time"),
    ("printf x | git commit -F -", "it is piped into `-F -`"),
    ("git commit", "it is written in an editor"),
    (
      "git commit -m x --trailer \"$T\"",
      "a `--trailer` value expands at run time",
    ),
    ("git commit -F sub", "its file `sub` is not a regular file"),
    (
      "git commit -F /dev/null",
      "its file `/dev/null` is not a regular file",
    ),
    ("git commit -F big.txt", "its file `big.txt` is over 64 KiB"),
    (
      "printf 'fix(x): y' > m.txt && git commit -F m.txt",
      "its file `m.txt` is written by the same command",
    ),
    (
      "cd sub && git commit -F m.txt",
      "its file `m.txt` is read after the line changes directory",
    ),
  ];
  for (source, why) in cases {
    assert_eq!(read(source, dir.path()), Err(why.to_owned()), "{source}");
  }
  let missing = read("git commit -F later.txt", dir.path()).unwrap_err();
  assert!(
    missing.starts_with("its file `later.txt` cannot be read yet ("),
    "{missing}"
  );
}
