//! The git facts the state layer asks (`state-git.ts`, `detect-branch.ts`,
//! `inLinkedWorktree`): the branch, the linked-worktree test, the common dir
//! and origin's HEAD are read from `.git` through `toolu_runtime::git`; git is
//! spawned only when the walk defers, and for the branch lists, which need
//! commit ancestry.

mod refs;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;
use toolu_runtime::git::{Discovery, discover};
use toolu_runtime::process::{Output, Spec, run};

pub use toolu_runtime::git::toplevel;

/// The symbolic ref naming origin's default branch.
const ORIGIN_HEAD: &str = "refs/remotes/origin/HEAD";

/// `git -C <root> rev-parse --abbrev-ref HEAD`, as TypeScript reads it: the
/// branch, `"HEAD"` when detached or unborn, `""` outside a repository.
pub fn current_branch(env: &Env, root: &Path) -> String {
  let read = match discover(env, root) {
    Discovery::Repo(repo) => refs::abbrev_head(&repo),
    Discovery::NotFound => Some(String::new()),
    Discovery::AskGit => None,
  };
  read.unwrap_or_else(|| {
    let out = git(env, root, &["rev-parse", "--abbrev-ref", "HEAD"]);
    out
      .map(|out| out.stdout.trim_end_matches('\n').to_owned())
      .unwrap_or_default()
  })
}

/// Whether `dir` sits in a linked worktree: its git dir and common dir differ.
pub fn linked_worktree(env: &Env, dir: &Path) -> bool {
  match discover(env, dir) {
    Discovery::Repo(repo) => repo.git_dir != repo.common_dir,
    Discovery::NotFound => false,
    Discovery::AskGit => {
      let args = [
        "rev-parse",
        "--path-format=absolute",
        "--git-dir",
        "--git-common-dir",
      ];
      let out = git(env, dir, &args)
        .map(|out| out.stdout)
        .unwrap_or_default();
      let mut lines = out.lines().map(|line| line.trim_end_matches('/'));
      match (lines.next(), lines.next()) {
        (Some(git_dir), Some(common)) => {
          !git_dir.is_empty() && !common.is_empty() && git_dir != common
        }
        _ => false,
      }
    }
  }
}

/// The common dir of the repository holding `dir`, or `None` outside one.
pub fn common_dir(env: &Env, dir: &Path) -> Option<PathBuf> {
  match discover(env, dir) {
    Discovery::Repo(repo) => Some(repo.common_dir),
    Discovery::NotFound => None,
    Discovery::AskGit => {
      let args = ["rev-parse", "--path-format=absolute", "--git-common-dir"];
      let out = git(env, dir, &args).filter(|out| out.exit_code == 0)?;
      let path = out.stdout.trim_end();
      (!path.is_empty()).then(|| PathBuf::from(path))
    }
  }
}

/// `detect_base_branch`: origin's HEAD branch, else `main`. Without a root
/// (or an empty one) it resolves the toplevel of `cwd`.
pub fn base_branch(env: &Env, root: Option<&Path>, cwd: &Path) -> String {
  let top = match root.filter(|root| !root.as_os_str().is_empty()) {
    Some(root) => root.to_path_buf(),
    None => match toplevel(env, cwd) {
      Some(top) => top,
      None => return "main".to_owned(),
    },
  };
  let target = match discover(env, &top) {
    Discovery::Repo(repo) => refs::symbolic_target(&repo, ORIGIN_HEAD),
    Discovery::NotFound => Some(String::new()),
    Discovery::AskGit => None,
  };
  let target = target.unwrap_or_else(|| {
    let out = git(env, &top, &["symbolic-ref", "--quiet", ORIGIN_HEAD]);
    let out = out.filter(|out| out.exit_code == 0);
    out
      .map(|out| out.stdout.trim().to_owned())
      .unwrap_or_default()
  });
  match target.as_str() {
    "" => "main".to_owned(),
    target => target
      .strip_prefix("refs/remotes/origin/")
      .unwrap_or(target)
      .to_owned(),
  }
}

/// `branch_slug`: `/` becomes `_`, then all but `[A-Za-z0-9_-]` is dropped;
/// empty is `_default`.
pub fn branch_slug(branch: &str) -> String {
  let slug: String = branch
    .chars()
    .map(|c| if c == '/' { '_' } else { c })
    .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
    .collect();
  if slug.is_empty() {
    "_default".to_owned()
  } else {
    slug
  }
}

/// Slugs of the local branches of `root`, only those merged into
/// `merged_into` when given (`git branch --merged`, which needs ancestry); empty
/// when git cannot list them.
pub fn branch_slugs(env: &Env, root: &Path, merged_into: Option<&str>) -> BTreeSet<String> {
  local_branches(env, root, merged_into).unwrap_or_default()
}

/// [`branch_slugs`], or `None` when `git branch` fails, so a caller can tell an
/// unlistable repository from one without branches.
pub fn local_branches(
  env: &Env,
  root: &Path,
  merged_into: Option<&str>,
) -> Option<BTreeSet<String>> {
  let mut args = vec!["branch", "--format=%(refname:short)"];
  if let Some(base) = merged_into {
    args.extend(["--merged", base]);
  }
  let out = git(env, root, &args).filter(|out| out.exit_code == 0)?;
  Some(
    out
      .stdout
      .lines()
      .filter(|name| !name.is_empty())
      .map(branch_slug)
      .collect(),
  )
}

/// `git --version` runs (bash `command -v git`).
pub fn has_git(env: &Env) -> bool {
  let mut spec = Spec::new(["git", "--version"]);
  spec.env = Some(env.clone());
  run(&spec).is_ok_and(|out| out.exit_code == 0)
}

/// `git -C <dir> <args>` with exactly `env`; `None` when git cannot start.
fn git(env: &Env, dir: &Path, args: &[&str]) -> Option<Output> {
  let mut spec = Spec::new(["git", "-C"]);
  spec.argv.push(dir.display().to_string());
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  spec.env = Some(env.clone());
  run(&spec).ok()
}

#[cfg(test)]
#[path = "tests/git_test.rs"]
mod tests;
