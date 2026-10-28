//! Real repositories for git-better's tests: a fixed identity and no system
//! config, so a developer's git setup never changes a result, and the hook
//! context a rule receives in them.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Map;
use tempfile::TempDir;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::Session;
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

/// `git <args>` in `root`, with `GIT_COMMITTER_DATE` when `date` is given; its stdout.
pub(crate) fn git_at(root: &Path, args: &[&str], date: Option<&str>) -> String {
  let mut command = Command::new("git");
  command
    .arg("-C")
    .arg(root)
    .args(args)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_AUTHOR_NAME", "t")
    .env("GIT_AUTHOR_EMAIL", "t@example.com")
    .env("GIT_COMMITTER_NAME", "t")
    .env("GIT_COMMITTER_EMAIL", "t@example.com");
  if let Some(date) = date {
    command.env("GIT_COMMITTER_DATE", date);
  }
  let output = command.output().unwrap();
  assert!(output.status.success(), "git {args:?}: {output:?}");
  String::from_utf8(output.stdout).unwrap()
}

/// `git <args>` in `root`; its stdout.
pub(crate) fn git(root: &Path, args: &[&str]) -> String {
  git_at(root, args, None)
}

/// A repository on `main` with one empty commit and a branch `feat/1-x`, and an
/// environment that reads only its own config.
pub(crate) struct Repo {
  _dir: TempDir,
  /// The work tree.
  pub(crate) root: PathBuf,
  /// The environment a hook or verb sees.
  pub(crate) env: Env,
}

impl Repo {
  /// A new repository; `config` is written as `<config_dir>/toolu.config.json`.
  pub(crate) fn new(config_dir: &str, config: Option<&str>) -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    std::fs::create_dir_all(root.join(config_dir)).unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["commit", "-q", "--allow-empty", "-m", "init"]);
    git(&root, &["branch", "feat/1-x"]);
    if let Some(config) = config {
      std::fs::write(root.join(config_dir).join("toolu.config.json"), config).unwrap();
    }
    let path = |name: &str| dir.path().join(name).display().to_string();
    let env = Env::from_pairs([
      ("PATH", std::env::var("PATH").unwrap_or_default()),
      ("HOME", dir.path().display().to_string()),
      ("GIT_CONFIG_NOSYSTEM", "1".to_owned()),
      ("TOOLU_CONFIG_DIR", path("user")),
      ("TOOLU_PROJECT_DIR", root.display().to_string()),
      ("TOOLU_SETTINGS_DIR", path("settings")),
    ]);
    Repo {
      _dir: dir,
      root,
      env,
    }
  }

  /// Write `file` and commit it with `message`, at `date` when given.
  pub(crate) fn commit(&self, file: &str, message: &str, date: Option<&str>) {
    let path = self.root.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, format!("{message}\n")).unwrap();
    git(&self.root, &["add", "-A"]);
    git_at(&self.root, &["commit", "-q", "-m", message], date);
  }

  /// The session a hook in this repository reports.
  pub(crate) fn session(&self, id: &str) -> Session {
    let root = Text::new(self.root.display().to_string()).unwrap();
    Session {
      session_id: Text::new(id).unwrap(),
      cwd: root.clone(),
      project_root: root.clone(),
      worktree: root,
    }
  }

  /// The context a rule receives on `host` in this repository.
  pub(crate) fn context<'a>(
    &'a self,
    host: Host,
    raw: &'a Map<String, serde_json::Value>,
  ) -> RuleContext<'a> {
    RuleContext {
      host,
      env: &self.env,
      config_root: &self.root,
      project_root: &self.root,
      cwd: Some(&self.root),
      plugin_root: None,
      raw,
      edit: None,
    }
  }
}

#[test]
fn a_repo_starts_on_main_with_a_feature_branch() {
  let repo = Repo::new(".claude", Some("{}"));
  assert_eq!(git(&repo.root, &["branch", "--show-current"]), "main\n");
  assert_eq!(
    git(&repo.root, &["branch", "--list", "feat/1-x"]),
    "  feat/1-x\n"
  );
  assert!(repo.root.join(".claude/toolu.config.json").is_file());
}
