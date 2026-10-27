//! Piped output stays plain: the rails, colors and progress of a terminal never
//! reach a pipe, so scripts and agents read `toolu plugins` and `toolu status`
//! as they always did.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::{Command, Output};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// `toolu <args>` in `dir` with `home` and a `bin/` holding stub hosts first on
/// `PATH`, stdout and stderr piped.
fn toolu(dir: &Path, home: &Path, args: &[&str]) -> Res<Output> {
  let path = format!("{}:/usr/bin:/bin", home.join("bin").display());
  Ok(
    Command::new(TOOLU)
      .args(args)
      .current_dir(dir)
      .env("HOME", home)
      .env("PATH", path)
      .env("TERM", "xterm-256color")
      .env_remove("NO_COLOR")
      .env_remove("CLAUDE_PROJECT_DIR")
      .env_remove("TOOLU_PROJECT_DIR")
      .output()?,
  )
}

/// A `claude` stub whose listing holds one toolu plugin.
fn claude(home: &Path) -> Res<()> {
  let bin = home.join("bin");
  std::fs::create_dir_all(&bin)?;
  let stub = bin.join("claude");
  std::fs::write(
    &stub,
    "#!/bin/sh\ncase \"$*\" in\n\"plugin list --json\") echo '[{\"id\":\"jev@toolu\",\"version\":\"8.5.0\"}]' ;;\nesac\n",
  )?;
  std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))?;
  Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Res<()> {
  let status = Command::new("git")
    .args([
      "-c",
      "user.name=t",
      "-c",
      "user.email=t@t",
      "-c",
      "commit.gpgsign=false",
    ])
    .args(args)
    .current_dir(dir)
    .env_remove("GIT_DIR")
    .env_remove("GIT_WORK_TREE")
    .status()?;
  if !status.success() {
    return Err(format!("git {}", args.join(" ")).into());
  }
  Ok(())
}

fn plain(output: &Output) -> bool {
  !output.stdout.contains(&0x1b) && !output.stderr.contains(&0x1b)
}

#[test]
fn a_piped_plugin_list_is_the_plain_marks() {
  let home = tempfile::tempdir().unwrap();
  claude(home.path()).unwrap();
  let output = toolu(
    home.path(),
    home.path(),
    &["plugins", "list", "--host", "claude"],
  )
  .unwrap();
  assert!(output.status.success(), "{output:?}");
  assert!(plain(&output), "{output:?}");
  let text = String::from_utf8_lossy(&output.stdout);
  assert!(text.contains("  = jev              8.5.0\n"), "{text}");
  assert!(!text.contains('┌') && !text.contains('◇'), "{text}");
}

#[test]
fn a_piped_status_is_the_json_document() {
  let root = tempfile::tempdir().unwrap();
  let repo = root.path().join("repo");
  std::fs::create_dir_all(&repo).unwrap();
  git(&repo, &["init", "-q", "-b", "main"]).unwrap();
  git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]).unwrap();
  let output = toolu(&repo, root.path(), &["--host", "claude", "status"]).unwrap();
  assert!(output.status.success(), "{output:?}");
  assert!(plain(&output), "{output:?}");
  let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  assert_eq!(document["branch"], "main");
}
