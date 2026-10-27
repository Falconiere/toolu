//! Plan-ledger test rig: a real repository with a linked worker worktree on
//! `feat/x`, and ledgers written by the code `toolu ledger run` and `toolu
//! ledger status` call.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;
use toolu_engine::ledger::commands::ledger_status;
use toolu_engine::ledger::context::RunFlags;
use toolu_engine::ledger::io::{LedgerOptions, ledger_path_for};
use toolu_engine::ledger::run::ledger_run;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::process::{Spec, run};

/// A passing and a failing step, so the summary has a red and a non-null `next`.
pub(crate) const STEPS: &str =
  r#"[{"id":"s1","title":"one","check":"true"},{"id":"s2","title":"two","check":"false"}]"#;

/// The main checkout and the worker's linked worktree.
pub(crate) struct Tree {
  _dir: tempfile::TempDir,
  /// The main checkout, where `.git` is a directory.
  pub(crate) main: PathBuf,
  /// The linked worktree, where `.git` is a `gitdir:` file.
  pub(crate) worktree: PathBuf,
}

impl Tree {
  /// `main` with one commit, and a worktree on `feat/x` one commit ahead.
  pub(crate) fn new() -> Tree {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = std::fs::canonicalize(dir.path()).expect("canonical temp");
    let main = root.join("main");
    let tree = root.join("worker");
    std::fs::create_dir_all(&main).expect("main dir");
    sh(
      &main,
      "git init -q -b main && echo base > base.txt && git add . && git commit -qm base \
       && git worktree add -q -b feat/x ../worker",
    );
    sh(&tree, "echo x > a.txt && git add a.txt && git commit -qm a");
    Tree {
      _dir: dir,
      main,
      worktree: tree,
    }
  }

  /// The worktree path as an issue record names it.
  pub(crate) fn path(&self) -> &str {
    self.worktree.to_str().expect("utf-8 path")
  }

  /// `bash -c script` in the worktree; returns its stdout.
  pub(crate) fn sh(&self, script: &str) -> String {
    sh(&self.worktree, script)
  }

  /// Run `steps` as the plan with `host`'s roots on the current branch, then
  /// `ledger status`; the ledger as status left it.
  pub(crate) fn ledger(&self, host: Host, steps: &str) -> Value {
    let body = format!("# P\n\n## Steps (machine-readable)\n\n```json\n{steps}\n```\n");
    std::fs::write(self.worktree.join("plan.md"), body).expect("plan doc");
    let opts = LedgerOptions {
      roots: Roots::new(env(&self.worktree), Some(host)),
      cwd: self.worktree.clone(),
      now: SystemTime::now,
    };
    let flags = RunFlags {
      only_step: None,
      activity: None,
      force: false,
      verify: false,
    };
    let ran = ledger_run("plan.md", &flags, &opts);
    assert!(ran.exit < 2, "ledger run: {}", ran.stderr);
    let (status, _) = ledger_status(&opts);
    let ledger = status.ledger.expect("status ledger");
    serde_json::from_str(&ledger.to_text(false)).expect("ledger json")
  }

  /// Rewrite `host`'s `feat/x` ledger in place through `change`.
  pub(crate) fn edit(&self, host: Host, change: impl FnOnce(&mut Value)) {
    let file = self.file(host, "feat/x");
    let text = std::fs::read_to_string(&file).expect("ledger");
    let mut ledger: Value = serde_json::from_str(&text).expect("ledger json");
    change(&mut ledger);
    std::fs::write(&file, ledger.to_string()).expect("rewrite ledger");
  }

  /// Where `host`'s ledger for `branch` lives in the worktree.
  pub(crate) fn file(&self, host: Host, branch: &str) -> PathBuf {
    ledger_path_for(
      &self.worktree,
      branch,
      &Roots::new(env(&self.worktree), Some(host)),
    )
  }
}

/// The ledger commands' environment. It keeps the runner's
/// `TOOLU_PROJECT_CONFIG_DIRNAME`, which the engine side reads from the process.
fn env(dir: &Path) -> Env {
  let path = std::env::var("PATH").unwrap_or_default();
  let home = dir.join("../.home").display().to_string();
  let dirname = std::env::var("TOOLU_PROJECT_CONFIG_DIRNAME").ok();
  let pairs = Env::from_pairs([
    ("PATH", path.as_str()),
    ("HOME", home.as_str()),
    ("TOOLU_CONFIG_DIR", home.as_str()),
    ("PUSH_REVIEW_BASE", "main"),
    ("GIT_AUTHOR_NAME", "t"),
    ("GIT_AUTHOR_EMAIL", "t@t"),
    ("GIT_COMMITTER_NAME", "t"),
    ("GIT_COMMITTER_EMAIL", "t@t"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
  ]);
  match dirname {
    Some(dirname) => pairs.with("TOOLU_PROJECT_CONFIG_DIRNAME", &dirname),
    None => pairs,
  }
}

/// `bash -c script` in `dir`; returns its stdout.
pub(crate) fn sh(dir: &Path, script: &str) -> String {
  let mut spec = Spec::new(["bash", "-c", script]);
  spec.cwd = Some(dir.to_path_buf());
  spec.env = Some(env(dir));
  let output = run(&spec).expect("bash");
  assert_eq!(output.exit_code, 0, "{script}: {}", output.stderr);
  output.stdout
}
