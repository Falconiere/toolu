//! `toolu code-review digest` against a real Git repository and the real ast-grep:
//! committed, uncommitted and untracked changes by file and item, `--since`,
//! `--path`, a clean branch, `--json`, and no ast-grep on PATH.

use std::error::Error;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

type Res<T> = Result<T, Box<dyn Error>>;
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

fn on_path(name: &str) -> Res<PathBuf> {
  let path = std::env::var_os("PATH").ok_or("PATH missing")?;
  std::env::split_paths(&path)
    .map(|dir| dir.join(name))
    .find(|path| path.is_file())
    .ok_or_else(|| format!("real {name} is required").into())
}

fn git(root: &Path, args: &[&str]) -> Res<()> {
  let status = Command::new("git")
    .args(["-c", "user.name=T", "-c", "user.email=t@e"])
    .args(args)
    .current_dir(root)
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .status()?;
  if status.success() {
    Ok(())
  } else {
    Err(format!("git {args:?} failed").into())
  }
}

/// `main` holds `src/lib.rs` and `Cargo.lock`; `feature` is checked out.
fn base() -> Res<tempfile::TempDir> {
  let dir = tempfile::tempdir()?;
  let root = dir.path();
  fs::create_dir(root.join("src"))?;
  fs::write(root.join("src/lib.rs"), "pub fn keep() -> u32 {\n  1\n}\n")?;
  fs::write(root.join("Cargo.lock"), "a\n")?;
  git(root, &["init", "-q", "-b", "main"])?;
  git(root, &["add", "-A"])?;
  git(root, &["commit", "-qm", "base"])?;
  git(root, &["switch", "-qc", "feature"])?;
  Ok(dir)
}

/// One commit on `feature`, then an uncommitted edit, an untracked file and a lockfile change.
fn changed() -> Res<tempfile::TempDir> {
  let dir = base()?;
  let root = dir.path();
  let committed = "pub fn keep() -> u32 {\n  1\n}\n\npub fn added() {}\n";
  fs::write(root.join("src/lib.rs"), committed)?;
  git(root, &["commit", "-qam", "add"])?;
  fs::write(root.join("src/lib.rs"), committed.replace("  1", "  2"))?;
  fs::write(root.join("src/new.rs"), "pub fn fresh() {}\n")?;
  fs::write(root.join("Cargo.lock"), "b\n")?;
  Ok(dir)
}

fn digest(root: &Path, args: &[&str], path: Option<&Path>) -> Res<Output> {
  let home = root.join(".home");
  fs::create_dir_all(&home)?;
  let mut command = Command::new(TOOLU);
  command
    .args(["code-review", "digest"])
    .args(args)
    .current_dir(root)
    .env("HOME", &home)
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env_remove("PUSH_REVIEW_BASE")
    .env_remove("TOOLU_PROJECT_DIR");
  if let Some(path) = path {
    command.env("PATH", path);
  }
  Ok(command.output()?)
}

fn stdout(out: &Output) -> String {
  assert_eq!(
    out.status.code(),
    Some(0),
    "{}",
    String::from_utf8_lossy(&out.stderr)
  );
  String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn digest_prints_counts_items_and_skipped_lockfile() {
  let dir = changed().unwrap();
  // The repository's own `.home` is ignored so it stays out of the snapshot.
  fs::write(dir.path().join(".git/info/exclude"), ".home/\n").unwrap();
  let text = stdout(&digest(dir.path(), &[], None).unwrap());
  assert!(text.starts_with("digest: main (merge-base "), "{text}");
  for want in [
    " + uncommitted, 2 files, +4 -1\n",
    "\nsrc/lib.rs +3 -1 (modified)\n  items: keep 1-3, added 5-5 (new)\n  @@ -2 +2 @@",
    "\n  -  1\n  +  2\n",
    "\n  +pub fn added() {}\n",
    "\nsrc/new.rs +1 -0 (added)\n  items: fresh 1-1 (new)\n  @@ -0,0 +1 @@\n  +pub fn fresh() {}\n",
  ] {
    assert!(text.contains(want), "missing {want:?} in:\n{text}");
  }
  assert!(
    text.ends_with("\nskipped: Cargo.lock (lockfile)\n"),
    "{text}"
  );
}

#[test]
fn digest_since_head_shows_local_changes_and_path_narrows() {
  let dir = changed().unwrap();
  fs::write(dir.path().join(".git/info/exclude"), ".home/\n").unwrap();
  let local = stdout(&digest(dir.path(), &["--since", "HEAD"], None).unwrap());
  assert!(local.starts_with("digest: HEAD (merge-base "), "{local}");
  assert!(
    local.contains(" + uncommitted, 2 files, +2 -1\n"),
    "{local}"
  );
  assert!(
    local.contains("\nsrc/lib.rs +1 -1 (modified)\n  items: keep 1-3\n"),
    "{local}"
  );
  assert!(!local.contains("added 5-5"), "{local}");
  let narrowed = stdout(&digest(dir.path(), &["--path", "src/new.rs"], None).unwrap());
  assert!(
    narrowed.contains(", 1 files, +1 -0\nsrc/new.rs +1 -0 (added)\n"),
    "{narrowed}"
  );
  assert!(!narrowed.contains("src/lib.rs"), "{narrowed}");
  assert!(!narrowed.contains("Cargo.lock"), "{narrowed}");
}

#[test]
fn digest_of_a_clean_branch_says_no_changes() {
  let dir = base().unwrap();
  fs::write(dir.path().join(".git/info/exclude"), ".home/\n").unwrap();
  let text = stdout(&digest(dir.path(), &[], None).unwrap());
  assert_eq!(text, "No changes against main\n");
}

#[test]
fn digest_json_is_one_document() {
  let dir = changed().unwrap();
  fs::write(dir.path().join(".git/info/exclude"), ".home/\n").unwrap();
  let out = digest(dir.path(), &["--json"], None).unwrap();
  let doc: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
  assert_eq!(doc["base"], "main");
  assert_eq!(doc["uncommitted"], true);
  assert_eq!(doc["files"][1]["path"], "src/new.rs");
  assert_eq!(doc["files"][1]["items"][0]["name"], "fresh");
  assert_eq!(doc["files"][1]["items"][0]["new"], true);
  assert_eq!(doc["skipped"][0]["reason"], "lockfile");
}

#[test]
fn digest_without_ast_grep_prints_items_unavailable() {
  let dir = changed().unwrap();
  fs::write(dir.path().join(".git/info/exclude"), ".home/\n").unwrap();
  let bin = dir.path().join(".home/bin");
  fs::create_dir_all(&bin).unwrap();
  symlink(on_path("git").unwrap(), bin.join("git")).unwrap();
  let text = stdout(&digest(dir.path(), &[], Some(&bin)).unwrap());
  assert!(
    text.contains("\nnote: items unavailable: ast-grep not found on PATH"),
    "{text}"
  );
  assert!(
    text.contains("\nsrc/new.rs +1 -0 (added)\n  items unavailable\n  @@ -0,0 +1 @@"),
    "{text}"
  );
}
