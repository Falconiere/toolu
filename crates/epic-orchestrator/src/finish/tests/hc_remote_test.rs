use crate::finish_rig::layout;
use serde_json::json;
use toolu_mux::hc::remote::{Target, Transfer};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

struct Cores {
  env: Env,
  _dir: tempfile::TempDir,
}

impl Cores {
  fn new() -> Result<Self, Box<dyn std::error::Error>> {
    let dir = tempfile::Builder::new().prefix("hc-").tempdir_in("/tmp")?;
    let root = dir.path().to_str().ok_or("non-UTF-8 path")?;
    let env = Env::process()
      .with("HC_SESSION", "local")
      .with("XDG_CONFIG_HOME", &format!("{root}/hc-c"))
      .with("XDG_DATA_HOME", &format!("{root}/hc-d"))
      .with("XDG_STATE_HOME", &format!("{root}/hc-s"))
      .with("XDG_RUNTIME_DIR", &format!("{root}/hc-r"));
    std::fs::create_dir_all(format!("{root}/hc-r"))?;
    let cores = Self { env, _dir: dir };
    let mut spec = Spec::new([
      "hc",
      "machine",
      "add",
      "--name",
      "loop",
      "--via",
      "/bin/sh -c",
      "--session",
      "remote",
      "--temp",
    ]);
    spec.env = Some(cores.env.clone());
    if run(&spec).map_err(|e| format!("{e:?}"))?.exit_code != 0 {
      return Err("hc machine setup failed".into());
    }
    Ok(cores)
  }

  fn transfer<'a>(&'a self, repo: &'a str) -> Transfer<'a> {
    Transfer {
      target: Target::new(Some("loop"), Some(repo)).unwrap().unwrap(),
      env: &self.env,
      run: "test",
    }
  }
}

impl Drop for Cores {
  fn drop(&mut self) {
    for session in ["local", "remote"] {
      let mut spec = Spec::new(["hc", "server", "stop"]);
      spec.env = Some(self.env.clone().with("HC_SESSION", session));
      let _stopped = run(&spec);
    }
  }
}

#[test]
fn remote_registration_checks_real_git_path_and_branch() {
  let l = layout();
  let output = std::process::Command::new("git")
    .args([
      "-C",
      l.main.to_str().expect("path"),
      "worktree",
      "list",
      "--porcelain",
      "-z",
    ])
    .output()
    .expect("git");
  assert!(output.status.success());
  let listing = std::str::from_utf8(&output.stdout).expect("utf8");
  let tree = l.wt.to_str().expect("path");
  assert_eq!(super::registered(listing, tree, "feat/1-x"), Ok(true));
  assert!(super::registered(listing, tree, "foreign").is_err());
  assert_eq!(
    super::registered(listing, "/missing", "feat/1-x"),
    Ok(false)
  );
}

#[test]
fn remote_removal_preserves_later_committed_work() {
  let l = layout();
  let cores = Cores::new().expect("hc setup");
  let repo = l.main.to_str().expect("repo");
  let tree = l.wt.to_str().expect("tree");
  let transfer = cores.transfer(repo);
  let intent = json!({"address":"loop/t1", "machine":"loop", "repo":repo, "worktree":tree, "branch":"feat/1-x"});
  let imported = crate::support::git(&l.wt, &["rev-parse", "HEAD"]);
  std::fs::write(l.wt.join("later"), "later work").expect("new work");
  crate::support::git(&l.wt, &["add", "later"]);
  crate::support::git(
    &l.wt,
    &[
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "-qm",
      "later",
    ],
  );
  let later = crate::support::git(&l.wt, &["rev-parse", "HEAD"]);
  let guarded = super::Teardown {
    path: &l.record,
    abandon: false,
    warnings: Vec::new(),
  };
  let mut protected = serde_json::Map::new();
  protected.insert("remote_result_commit".into(), json!(imported.trim()));
  assert!(
    super::remove(&transfer, &guarded, &mut protected, &intent)
      .unwrap_err()
      .contains("untransferred commits")
  );
  assert!(l.wt.exists());
  assert_eq!(
    crate::support::git(&l.main, &["rev-parse", "refs/heads/feat/1-x"]),
    later
  );
}

#[test]
fn interrupted_remote_removal_is_reconciled_only_with_matching_intent() {
  let l = layout();
  let cores = Cores::new().expect("hc setup");
  let repo = l.main.to_str().expect("repo");
  let tree = l.wt.to_str().expect("tree");
  let transfer = cores.transfer(repo);
  let teardown = super::Teardown {
    path: &l.record,
    abandon: true,
    warnings: Vec::new(),
  };
  let intent = json!({"address":"loop/t1", "machine":"loop", "repo":repo, "worktree":tree, "branch":"feat/1-x"});
  crate::support::git(&l.main, &["worktree", "remove", tree]);
  let mut record = serde_json::Map::new();
  assert!(super::remove(&transfer, &teardown, &mut record, &intent).is_err());
  record.insert("remote_removal_intent".into(), intent.clone());
  std::fs::create_dir(&l.wt).expect("foreign path");
  assert!(super::remove(&transfer, &teardown, &mut record, &intent).is_err());
  std::fs::remove_dir(&l.wt).expect("remove foreign path");
  super::remove(&transfer, &teardown, &mut record, &intent).expect("reconcile");
  assert_eq!(record["remote_worktree_removed"], true);
}
