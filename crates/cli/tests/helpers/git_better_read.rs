//! `toolu git-better log`, `context` and `pr` from the real binary on a real
//! feature branch, against `$defs/gitBetterLog`, `gitBetterContext` and
//! `gitBetterPr`, and the `SessionStart` summary hook.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

use crate::cli::{Res, TOOLU, one_document};
use crate::schema::violations;

fn git(root: &Path, args: &[&str]) -> Res<()> {
  let status = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_AUTHOR_NAME", "t")
    .env("GIT_AUTHOR_EMAIL", "t@example.com")
    .env("GIT_COMMITTER_NAME", "t")
    .env("GIT_COMMITTER_EMAIL", "t@example.com")
    .status()?;
  if !status.success() {
    return Err(format!("git {args:?} failed").into());
  }
  Ok(())
}

/// `feat/653-pause-it` with two semantic commits ahead of `main`.
fn branch(home: &Path) -> Res<std::path::PathBuf> {
  let root = home.join("repo");
  std::fs::create_dir_all(root.join("crates/epic/src"))?;
  git(&root, &["init", "-q", "-b", "main"])?;
  git(&root, &["commit", "-q", "--allow-empty", "-m", "init"])?;
  git(&root, &["switch", "-q", "-c", "feat/653-pause-it"])?;
  std::fs::write(root.join("crates/epic/src/pause.rs"), "a\nb\n")?;
  git(&root, &["add", "-A"])?;
  let first = "feat(epic): add pause\n\nIntent: workers can wait\nRisk: low";
  git(&root, &["commit", "-q", "-m", first])?;
  std::fs::write(root.join("crates/epic/src/resume.rs"), "c\n")?;
  git(&root, &["add", "-A"])?;
  let second = "feat(epic): add resume\n\nIntent: waiting workers continue\nRisk: medium";
  git(&root, &["commit", "-q", "-m", second])?;
  Ok(root)
}

fn toolu(dir: &Path, home: &Path, args: &[&str]) -> Res<Output> {
  Ok(
    Command::new(TOOLU)
      .args(args)
      .current_dir(dir)
      .env("HOME", home)
      .env("GIT_CONFIG_NOSYSTEM", "1")
      .env("TOOLU_CONFIG_DIR", home.join("config"))
      .env("TOOLU_SETTINGS_DIR", home.join("settings"))
      .output()?,
  )
}

fn document(dir: &Path, home: &Path, args: &[&str], def: &str) -> Res<Value> {
  let output = toolu(dir, home, args)?;
  if output.status.code() != Some(0) {
    return Err(format!("{args:?}: {output:?}").into());
  }
  let document = one_document(&output)?;
  let found = violations(&document, Some(def))?;
  if !found.is_empty() {
    return Err(format!("{document}: {found:?}").into());
  }
  Ok(document)
}

#[test]
fn log_context_and_pr_are_valid_documents_about_the_branch() {
  let home = tempfile::tempdir().unwrap();
  let root = branch(home.path()).unwrap();
  let log = document(
    &root,
    home.path(),
    &["--json", "git-better", "log"],
    "gitBetterLog",
  )
  .unwrap();
  assert_eq!(log["range"], "refs/heads/main..HEAD");
  assert_eq!(log["commits"][0]["intent"], "waiting workers continue");
  assert_eq!(log["commits"][1]["areas"][0]["area"], "crates/epic");
  let one = ["--json", "git-better", "log", "main..HEAD", "--limit", "1"];
  let limited = document(&root, home.path(), &one, "gitBetterLog").unwrap();
  assert_eq!(limited["commits"].as_array().map(Vec::len), Some(1));
  let context_args = ["--json", "git-better", "context"];
  let context = document(&root, home.path(), &context_args, "gitBetterContext").unwrap();
  assert_eq!(context["risk"], "medium");
  assert_eq!(context["onBase"], false);
  let pr = document(
    &root,
    home.path(),
    &["--json", "git-better", "pr"],
    "gitBetterPr",
  )
  .unwrap();
  assert_eq!(pr["title"], "feat(epic): pause it");
  assert!(pr["body"].as_str().unwrap().ends_with("Closes #653"));
  let squash = toolu(&root, home.path(), &["git-better", "pr", "--squash"]).unwrap();
  let text = String::from_utf8(squash.stdout).unwrap();
  assert!(text.contains("\nIntent: workers can wait; waiting workers continue\nRisk: medium"));
}

#[test]
fn the_session_summary_names_the_branch_and_outside_a_repository_reads_fail() {
  let home = tempfile::tempdir().unwrap();
  let root = branch(home.path()).unwrap();
  let args = [
    "git-better",
    "hook",
    "session-start",
    "--event",
    "SessionStart",
  ];
  let output = toolu(&root, home.path(), &args).unwrap();
  assert_eq!(output.status.code(), Some(0), "{output:?}");
  let hook: Value = serde_json::from_slice(&output.stdout).unwrap();
  let text = hook["hookSpecificOutput"]["additionalContext"]
    .as_str()
    .unwrap();
  assert!(
    text.contains("Branch feat/653-pause-it (feat, ref 653: pause it): 2 commits ahead of main"),
    "{text}"
  );
  let outside = toolu(home.path(), home.path(), &["git-better", "context"]).unwrap();
  assert_eq!(outside.status.code(), Some(1), "{outside:?}");
}
