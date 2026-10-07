//! A sandboxed repository on `feat/x`, one commit ahead of `main`, shared by
//! the ledger's unit tests: `sh` runs a script in it, `opts` is the Claude host
//! with `PUSH_REVIEW_BASE=main` and the clock fixed at 2031-02-03T04:05:06Z.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::process::{Spec, run};

use crate::ledger::io::{LedgerOptions, head_branch};
use crate::verdict::gates::GateContext;

/// A helper's result; the tests unwrap it.
pub(crate) type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// The repository and the temp dir that holds it.
pub(crate) struct Repo {
  _dir: tempfile::TempDir,
  pub(crate) root: PathBuf,
}

fn fixed() -> SystemTime {
  UNIX_EPOCH + Duration::from_secs(1_927_857_906)
}

impl Repo {
  /// `main` with `base.txt`, then `feat/x` adding `a.ts`.
  pub(crate) fn new() -> Res<Repo> {
    let dir = tempfile::tempdir()?;
    let root = std::fs::canonicalize(dir.path())?.join("project");
    std::fs::create_dir_all(root.join(".home"))?;
    let repo = Repo { _dir: dir, root };
    repo.sh(
      "git init -q -b main && echo base > base.txt && git add . && git commit -qm base \
       && git checkout -q -b feat/x && echo x > a.ts && git add a.ts && git commit -qm a",
    )?;
    Ok(repo)
  }

  /// The environment every command of a test sees.
  pub(crate) fn env(&self) -> Env {
    let path = std::env::var("PATH").unwrap_or_default();
    let home = self.root.join(".home").display().to_string();
    Env::from_pairs([
      ("PATH", path.as_str()),
      ("HOME", home.as_str()),
      ("TOOLU_CONFIG_DIR", home.as_str()),
      ("TOOLU_HOST_OVERRIDE", "claude"),
      ("PUSH_REVIEW_BASE", "main"),
      ("GIT_AUTHOR_NAME", "t"),
      ("GIT_AUTHOR_EMAIL", "t@t"),
      ("GIT_COMMITTER_NAME", "t"),
      ("GIT_COMMITTER_EMAIL", "t@t"),
      ("GIT_CONFIG_NOSYSTEM", "1"),
    ])
  }

  /// `bash -c script` in the repository; a failing script is an error.
  pub(crate) fn sh(&self, script: &str) -> Res<String> {
    let mut spec = Spec::new(["bash", "-c", script]);
    spec.cwd = Some(self.root.clone());
    spec.env = Some(self.env());
    let output = run(&spec).map_err(|err| format!("{err:?}"))?;
    if output.exit_code != 0 {
      return Err(format!("{script}: {}", output.stderr).into());
    }
    Ok(output.stdout)
  }

  /// A verdict gate's context at the root: `branch` against `main`, at `cur`.
  pub(crate) fn gate<'a>(&self, roots: &'a Roots, branch: &str, cur: &str) -> GateContext<'a> {
    GateContext {
      roots,
      root: self.root.clone(),
      branch: branch.to_owned(),
      base: "main".to_owned(),
      cur: cur.to_owned(),
      cwd: self.root.clone(),
      warnings: Vec::new(),
    }
  }

  /// The ledger options of a command run at the root.
  pub(crate) fn opts(&self) -> LedgerOptions {
    LedgerOptions {
      roots: Roots::new(self.env(), Some(Host::Claude)),
      cwd: self.root.clone(),
      now: fixed,
    }
  }
}

#[test]
fn the_repository_is_one_commit_ahead_of_main_on_the_claude_host() {
  let repo = Repo::new().unwrap();
  assert_eq!(repo.sh("git rev-list --count main..HEAD").unwrap(), "1\n");
  let opts = repo.opts();
  assert_eq!(
    head_branch(opts.env(), &opts.cwd).as_deref(),
    Some("feat/x")
  );
  assert_eq!(opts.roots.host(), Host::Claude);
}
