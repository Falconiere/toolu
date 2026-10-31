//! A tmux client on a fresh socket, shared by this crate's own tests.

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use crate::tmux::Tmux;

/// A PATH wide enough to find `git` and `tmux` on the usual dev machines.
const PATH: &str = "/usr/bin:/bin:/opt/homebrew/bin";

/// A tmux client on a fresh socket, with a scratch directory for worktrees.
pub(crate) struct World {
  _dir: tempfile::TempDir,
  pub(crate) client: Tmux,
  pub(crate) repo: PathBuf,
}

impl World {
  pub(crate) fn new() -> World {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("tmux.sock");
    let env = Env::from_pairs([("PATH", PATH)]);
    let client = Tmux::new(&env, Some(socket));
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    World {
      _dir: dir,
      client,
      repo,
    }
  }

  /// A git repository with one committed file on `main`.
  pub(crate) fn git_repo(&self) {
    let repo = &self.repo;
    run_git(repo, &["init", "-q"]);
    run_git(repo, &["config", "user.email", "test@example.com"]);
    run_git(repo, &["config", "user.name", "Test"]);
    std::fs::write(repo.join("file.txt"), "hello\n").unwrap();
    run_git(repo, &["add", "file.txt"]);
    run_git(repo, &["commit", "-q", "-m", "init"]);
  }
}

impl Drop for World {
  /// Removing the scratch directory alone leaves the server running forever.
  fn drop(&mut self) {
    let _no_server = self.client.kill_server();
  }
}

fn run_git(cwd: &Path, args: &[&str]) {
  let mut command = vec![
    "git".to_owned(),
    "-C".to_owned(),
    cwd.to_string_lossy().into_owned(),
  ];
  command.extend(args.iter().map(|arg| (*arg).to_owned()));
  let mut spec = Spec::new(command);
  spec.env = Some(Env::from_pairs([("PATH", PATH)]));
  let output = run(&spec).unwrap();
  assert_eq!(output.exit_code, 0, "git {args:?}: {}", output.stderr);
}
