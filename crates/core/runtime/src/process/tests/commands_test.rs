use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use super::{codex_plugin_list, git_toplevel, is_git_repo};
use crate::env::Env;

fn path_env() -> Env {
  Env::from_pairs([("PATH", std::env::var("PATH").unwrap())])
}

fn git(dir: &Path, args: &[&str]) {
  let status = std::process::Command::new("git")
    .args(args)
    .current_dir(dir)
    .status()
    .unwrap();
  assert!(status.success());
}

#[test]
fn the_toplevel_of_a_real_repository_is_found_from_a_subdirectory() {
  let dir = tempfile::tempdir().unwrap();
  git(dir.path(), &["init", "-q"]);
  std::fs::create_dir(dir.path().join("src")).unwrap();
  let top = git_toplevel(&path_env(), &dir.path().join("src")).unwrap();
  assert_eq!(top, std::fs::canonicalize(dir.path()).unwrap());
}

#[test]
fn a_repository_is_recognised_by_its_git_dir() {
  let dir = tempfile::tempdir().unwrap();
  let env = path_env().with("GIT_CEILING_DIRECTORIES", &dir.path().display().to_string());
  assert!(!is_git_repo(&env, dir.path()));
  git(dir.path(), &["init", "-q"]);
  assert!(is_git_repo(&env, dir.path()));
}

#[test]
fn outside_a_repository_there_is_no_toplevel() {
  let dir = tempfile::tempdir().unwrap();
  let ceiling = path_env().with("GIT_CEILING_DIRECTORIES", &dir.path().display().to_string());
  assert_eq!(git_toplevel(&ceiling, dir.path()), None);
}

#[test]
fn without_git_on_path_there_is_no_toplevel() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("PATH", dir.path().display().to_string())]);
  assert_eq!(git_toplevel(&env, dir.path()), None);
}

fn fake_codex(dir: &Path, script: &str) -> Env {
  let bin = dir.join("codex");
  std::fs::write(&bin, format!("#!/bin/sh\n{script}\n")).unwrap();
  std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
  let path = format!("{}:{}", dir.display(), std::env::var("PATH").unwrap());
  Env::from_pairs([("PATH", path)])
}

#[test]
fn the_codex_listing_is_its_stdout_on_success_only() {
  let dir = tempfile::tempdir().unwrap();
  let env = fake_codex(dir.path(), r#"echo "$1 $2 $3""#);
  assert_eq!(
    codex_plugin_list(&env).as_deref(),
    Some("plugin list --json\n")
  );
  let failing = fake_codex(dir.path(), "echo partial; exit 1");
  assert_eq!(codex_plugin_list(&failing), None);
  let empty = tempfile::tempdir().unwrap();
  let missing = Env::from_pairs([("PATH", empty.path().display().to_string())]);
  assert_eq!(codex_plugin_list(&missing), None);
}
