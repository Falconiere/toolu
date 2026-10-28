use std::fs;
use std::process::Command;

use toolu_runtime::env::Env;

use super::{files, root};

#[test]
fn root_resolves_the_real_repository_from_the_crate_directory() {
  let env = Env::process();
  let repo = root(&env, env!("CARGO_MANIFEST_DIR")).unwrap();
  assert!(repo.join("Cargo.toml").is_file());
  assert!(repo.join("crates/toolu-review/Cargo.toml").is_file());
}

/// The push gate compares `reviewed_files` with Git's quoted `--name-only` lines.
#[test]
fn reviewed_files_keep_the_gate_spelling_of_unusual_paths() {
  let dir = tempfile::tempdir().expect("repo");
  let git = |args: &[&str]| {
    let out = Command::new("git")
      .arg("-C")
      .arg(dir.path())
      .args(args)
      .env("GIT_CONFIG_NOSYSTEM", "1")
      .env("GIT_CONFIG_GLOBAL", "/dev/null")
      .env("XDG_CONFIG_HOME", "/dev/null")
      .output()
      .expect("git starts");
    assert!(
      out.status.success(),
      "git {args:?}: {}",
      String::from_utf8_lossy(&out.stderr)
    );
  };
  git(&["init", "-b", "main"]);
  git(&["config", "user.name", "Test"]);
  git(&["config", "user.email", "test@example.com"]);
  fs::write(dir.path().join("base.txt"), "base\n").expect("base file");
  git(&["add", "-A"]);
  git(&["commit", "-m", "base"]);
  git(&["switch", "-c", "feature"]);
  fs::write(dir.path().join("a\nb.rs"), "one\n").expect("newline path");
  fs::write(dir.path().join("café.rs"), "two\n").expect("unicode path");
  git(&["add", "-A"]);
  git(&["commit", "-m", "feature"]);
  let env = Env::process()
    .with("GIT_CONFIG_NOSYSTEM", "1")
    .with("GIT_CONFIG_GLOBAL", "/dev/null")
    .with("XDG_CONFIG_HOME", "/dev/null");
  assert_eq!(
    files(&env, dir.path(), "main", None).expect("paths"),
    [r#""a\nb.rs""#, r#""caf\303\251.rs""#]
  );
  assert_eq!(
    files(&env, dir.path(), "main", Some("x,y")).expect("override"),
    ["x", "y"]
  );
}
