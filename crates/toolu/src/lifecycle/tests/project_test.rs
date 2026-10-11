use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use toolu_runtime::env::Env;

use super::{branch_line, git_toplevel, on_path, project_facts};

#[test]
fn command_lookup_requires_an_executable_file() {
  let dir = tempfile::tempdir().unwrap();
  let bin = dir.path().join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  let tool = bin.join("sg");
  std::fs::write(&tool, "").unwrap();
  assert!(!on_path("sg", bin.to_str().unwrap()));
  let mut perms = std::fs::metadata(&tool).unwrap().permissions();
  perms.set_mode(0o755);
  std::fs::set_permissions(&tool, perms).unwrap();
  let path = bin.to_str().unwrap();
  assert!(on_path("sg", path));
  assert!(!on_path("git", path));
  assert!(!on_path("a/b", path));
}

#[test]
fn facts_come_from_the_toplevel_and_an_empty_root_is_blank() {
  let dir = tempfile::tempdir().unwrap();
  let repo = dir.path().join("repo");
  std::fs::create_dir_all(&repo).unwrap();
  git(&repo, &["init", "-b", "feature"]);
  let ceiling = std::fs::canonicalize(dir.path()).unwrap();
  let env = Env::from_pairs([
    ("PATH", "/usr/bin:/bin"),
    ("GIT_CEILING_DIRECTORIES", ceiling.to_str().unwrap()),
  ]);
  assert_eq!(branch_line(&env, &repo).as_deref(), Some("Branch: HEAD"));
  let outside_dir = dir.path().join("outside");
  std::fs::create_dir_all(&outside_dir).unwrap();
  assert_eq!(branch_line(&env, &outside_dir), None);
  std::fs::write(repo.join("bun.lock"), "").unwrap();
  std::fs::write(repo.join("Cargo.toml"), "").unwrap();
  std::fs::write(repo.join("tsconfig.json"), "{}").unwrap();
  git(&repo, &["add", "."]);
  git(&repo, &["commit", "-m", "init"]);
  let facts = project_facts(Some(&repo), &env, true);
  assert_eq!(facts.name, "repo");
  assert_eq!(facts.node_pm, "bun");
  assert!(facts.rust);
  assert!(facts.ts);
  assert!(!facts.python);
  let quiet = project_facts(Some(&repo), &env, false);
  assert!(!quiet.ts);
  let outside = project_facts(None, &env, true);
  assert_eq!(outside.name, "");
  let canonical = std::fs::canonicalize(&repo).unwrap();
  assert_eq!(git_toplevel(&env, &repo), canonical.to_str().unwrap());
  assert_eq!(branch_line(&env, &repo).as_deref(), Some("Branch: feature"));
  let blind = Env::from_pairs([("PATH", dir.path().to_str().unwrap())]);
  assert_eq!(branch_line(&blind, &repo), None);
  assert_eq!(git_toplevel(&blind, &repo), "");
}

fn git(repo: &Path, args: &[&str]) {
  let status = Command::new("git")
    .current_dir(repo)
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_AUTHOR_NAME", "toolu")
    .env("GIT_AUTHOR_EMAIL", "toolu@example.com")
    .env("GIT_COMMITTER_NAME", "toolu")
    .env("GIT_COMMITTER_EMAIL", "toolu@example.com")
    .args(args)
    .status()
    .unwrap();
  assert!(status.success(), "{args:?}");
}
