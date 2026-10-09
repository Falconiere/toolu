//! Real Git facts for review state, through the runtime's bounded process owner.

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};
use toolu_state::git::current_branch;

fn git(env: &Env, dir: &Path, args: &[&str]) -> Option<String> {
  let mut spec = Spec::new(["git", "-C", &dir.display().to_string()]);
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  spec.env = Some(env.clone());
  run(&spec)
    .ok()
    .filter(|out| out.exit_code == 0 && !out.truncated)
    .map(|out| out.stdout)
}

/// Absolute Git toplevel for the requested path.
pub(crate) fn root(env: &Env, repo: &str) -> Result<PathBuf, String> {
  let top = git(env, Path::new(repo), &["rev-parse", "--show-toplevel"])
    .ok_or_else(|| format!("{repo} is not inside a git repo"))?;
  let top = top.trim();
  if top.is_empty() {
    return Err(format!("{repo} is not inside a git repo"));
  }
  Ok(PathBuf::from(top))
}

/// Attached checkout branch, or a validated existing target for detached HEAD.
pub(crate) fn branch(env: &Env, root: &Path, requested: Option<&str>) -> Result<String, String> {
  let current = current_branch(env, root);
  if !current.is_empty() && current != "HEAD" {
    if requested.is_some_and(|requested| requested != current) {
      return Err(format!(
        "--branch '{}' does not match the checked-out branch '{current}'",
        requested.unwrap_or_default()
      ));
    }
    return Ok(current);
  }
  let requested = requested.ok_or_else(|| {
    "not on a branch (detached HEAD?) — pass --branch <name> naming the branch the push targets"
      .to_owned()
  })?;
  if git(env, root, &["check-ref-format", "--branch", requested]).is_none() {
    return Err(format!("--branch '{requested}' is not a valid branch name"));
  }
  let known = ["refs/heads", "refs/remotes/origin"].iter().any(|prefix| {
    git(
      env,
      root,
      &[
        "show-ref",
        "--verify",
        "--quiet",
        &format!("{prefix}/{requested}"),
      ],
    )
    .is_some()
  });
  if !known {
    return Err(format!("unknown branch '{requested}'"));
  }
  Ok(requested.to_owned())
}

/// Paths covered by the review, sorted and deduplicated in UTF-8 byte order.
pub(crate) fn files(
  env: &Env,
  root: &Path,
  base: &str,
  override_files: Option<&str>,
) -> Result<Vec<String>, String> {
  let names = match override_files {
    Some(names) => names.to_owned(),
    None => git(
      env,
      root,
      &[
        "diff",
        "--no-color",
        &format!("{base}...HEAD"),
        "--name-only",
      ],
    )
    .ok_or_else(|| "failed to compute reviewed_files".to_owned())?,
  };
  let sep = if override_files.is_some() { ',' } else { '\n' };
  let mut files: Vec<String> = names
    .split(sep)
    .filter(|name| !name.is_empty())
    .map(str::to_owned)
    .collect();
  files.sort();
  files.dedup();
  Ok(files)
}

#[cfg(test)]
#[path = "tests/git_test.rs"]
mod tests;
