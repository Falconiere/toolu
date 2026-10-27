//! Black-box shape contract against a real committed Git diff.

use std::fs;
use std::path::Path;
use std::process::Command;

use toolu_code_review::shape::{ChangedLine, Chunk, Shaped, ShapedFile, shape};
use toolu_runtime::env::Env;

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
  let out = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .env("XDG_CONFIG_HOME", "/dev/null")
    .output()
    .map_err(|err| format!("git {args:?}: {err}"))?;
  if !out.status.success() {
    return Err(format!(
      "git {args:?}: {}",
      String::from_utf8_lossy(&out.stderr)
    ));
  }
  Ok(())
}

#[test]
fn public_shape_contract_cites_committed_lines() {
  let dir = tempfile::tempdir().expect("repository");
  git(dir.path(), &["init", "-b", "main"]).expect("init");
  git(dir.path(), &["config", "user.name", "Test"]).expect("name");
  git(dir.path(), &["config", "user.email", "test@example.com"]).expect("email");
  fs::write(dir.path().join("source.rs"), "before\n").expect("base");
  git(dir.path(), &["add", "-A"]).expect("add base");
  git(dir.path(), &["commit", "-m", "base"]).expect("commit base");
  git(dir.path(), &["switch", "-c", "feature"]).expect("branch");
  fs::write(dir.path().join("source.rs"), "after\n").expect("change");
  git(dir.path(), &["add", "-A"]).expect("add change");
  git(dir.path(), &["commit", "-m", "change"]).expect("commit change");

  let env = Env::process()
    .with("GIT_CONFIG_NOSYSTEM", "1")
    .with("GIT_CONFIG_GLOBAL", "/dev/null")
    .with("XDG_CONFIG_HOME", "/dev/null");
  let shaped: Shaped = shape(&env, dir.path(), Some("main")).expect("shape");
  let file: &ShapedFile = &shaped.files[0];
  let changed: &ChangedLine = &file.lines[1];
  let chunk: &Chunk = &shaped.chunks[0];
  assert_eq!(shaped.coverage.reviewed, ["source.rs"]);
  assert_eq!(file.path, "source.rs");
  assert_eq!((changed.line, changed.text.as_str()), (1, "after"));
  assert_eq!(chunk.files, [0]);
}
