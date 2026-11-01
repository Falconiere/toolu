//! Real repositories for the sweep's tests: a bare origin, a clone whose
//! `origin/HEAD` is `main`, and an environment whose `PATH` has git but no
//! `herdr`, `colima`, `docker` or `toolu`, so no test touches a developer's VM.

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use super::Request;

/// Git and nothing else.
pub(crate) const PATH: &str = "/usr/bin:/bin";

/// The environment every test command and sweep runs with.
pub(crate) fn env(home: &Path) -> Env {
  Env::from_pairs([
    ("PATH", PATH),
    ("HOME", &home.display().to_string()),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_AUTHOR_NAME", "t"),
    ("GIT_AUTHOR_EMAIL", "t@t"),
    ("GIT_COMMITTER_NAME", "t"),
    ("GIT_COMMITTER_EMAIL", "t@t"),
  ])
}

/// `git -C dir args`, failing with git's stderr.
pub(crate) fn git(env: &Env, dir: &Path, args: &[&str]) -> Result<String, String> {
  let mut spec = Spec::new(["git", "-C", &dir.display().to_string()]);
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  spec.env = Some(env.clone());
  let output = run(&spec).map_err(|err| format!("{err:?}"))?;
  if output.exit_code == 0 {
    Ok(output.stdout)
  } else {
    Err(format!("git {args:?}: {}", output.stderr))
  }
}

/// A sandbox: the temp dir, a bare `origin`, and a clone at `<tmp>/repo` with
/// one commit on `main` pushed and `origin/HEAD` set.
pub(crate) struct Repo {
  pub(crate) _dir: tempfile::TempDir,
  pub(crate) tmp: PathBuf,
  pub(crate) root: PathBuf,
  pub(crate) env: Env,
}

impl Repo {
  pub(crate) fn new() -> Result<Repo, String> {
    let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
    let tmp = std::fs::canonicalize(dir.path()).map_err(|err| err.to_string())?;
    let env = env(&tmp);
    let origin = tmp.join("origin.git");
    let root = tmp.join("repo");
    git(
      &env,
      &tmp,
      &["init", "-q", "--bare", "-b", "main", "origin.git"],
    )?;
    git(
      &env,
      &tmp,
      &["clone", "-q", &origin.display().to_string(), "repo"],
    )?;
    std::fs::write(root.join("README"), "one\n").map_err(|err| err.to_string())?;
    git(&env, &root, &["add", "README"])?;
    git(&env, &root, &["commit", "-q", "-m", "init"])?;
    git(&env, &root, &["push", "-q", "-u", "origin", "main"])?;
    git(&env, &root, &["remote", "set-head", "origin", "main"])?;
    Ok(Repo {
      _dir: dir,
      tmp,
      root,
      env,
    })
  }

  /// A linked worktree at `path` on new branch `branch` from `main`.
  pub(crate) fn worktree(&self, path: &Path, branch: &str) -> Result<(), String> {
    git(
      &self.env,
      &self.root,
      &[
        "worktree",
        "add",
        "-q",
        "-b",
        branch,
        &path.display().to_string(),
        "main",
      ],
    )
    .map(drop)
  }

  /// One commit in `dir`, so its branch is ahead of `main`.
  pub(crate) fn commit(&self, dir: &Path, file: &str) -> Result<(), String> {
    std::fs::write(dir.join(file), "x\n").map_err(|err| err.to_string())?;
    git(&self.env, dir, &["add", file])?;
    git(&self.env, dir, &["commit", "-q", "-m", file]).map(drop)
  }

  /// A sweep request over this clone with `apply`.
  pub(crate) fn request(&self, apply: bool) -> Request {
    Request {
      repos: vec![self.root.clone()],
      state_dirs: vec![self.tmp.join("epic")],
      resource_root: self.tmp.join("resources"),
      toolu: None,
      apply,
      all_branches: true,
      env: self.env.clone(),
    }
  }

  /// An issue record under `<tmp>/epic/issues/<key>.json`.
  pub(crate) fn record(&self, key: &str, stage: &str, leftovers: &[&str]) -> Result<(), String> {
    let issues = self.tmp.join("epic").join("issues");
    std::fs::create_dir_all(&issues).map_err(|err| err.to_string())?;
    let doc = serde_json::json!({
      "key": key,
      "stage": stage,
      "branch": format!("feat/{key}"),
      "checkout": self.root,
      "worktree": self.tmp.join(".herdr/worktrees/repo").join(key),
      "leftover_files": leftovers,
    });
    std::fs::write(issues.join(format!("{key}.json")), doc.to_string())
      .map_err(|err| err.to_string())
  }
}
