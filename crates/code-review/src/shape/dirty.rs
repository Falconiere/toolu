//! Ignore only this plugin's own state in the dirty-worktree probe.

use std::path::Path;
use toolu_runtime::env::Env;

use super::git_io::git_output;
use super::{STATUS_PROBE_BYTES, ShapeError};

/// The state entries a review run writes itself.
const OWN_STATE: [&str; 4] = [
  "code-review",
  "code-review-usage.jsonl",
  "code-review-usage.jsonl.lock",
  "push-review",
];

/// Whether Git sees changes besides the review plugin's own state files.
pub(crate) fn dirty_except_state(
  env: &Env,
  root: &Path,
  state_root: &Path,
) -> Result<bool, ShapeError> {
  // Pathspecs resolve against `root`, which may sit below the toplevel that
  // porcelain paths are relative to; Git also keeps own state out of the bounded probe.
  let mut excludes = Vec::new();
  if let Ok(prefix) = state_root.strip_prefix(root) {
    for name in OWN_STATE {
      let Some(path) = prefix.join(name).to_str().map(str::to_owned) else {
        return Ok(true);
      };
      excludes.push(format!(":(exclude,literal){path}"));
    }
    let Some(archives) = archive_excludes(env, root, state_root, prefix)? else {
      return Ok(true);
    };
    excludes.extend(archives);
  }
  dirty_with_excludes(env, root, &excludes)
}

fn archive_excludes(
  env: &Env,
  root: &Path,
  state_root: &Path,
  prefix: &Path,
) -> Result<Option<Vec<String>>, ShapeError> {
  let archive_dir = prefix.join("code-review-usage");
  let Some(archive_dir_str) = archive_dir.to_str() else {
    return Ok(None);
  };
  let (tracked, truncated) = git_output(
    env,
    root,
    &[
      "--no-optional-locks",
      "status",
      "--porcelain=v1",
      "-z",
      "--untracked-files=no",
      "--",
      &format!(":(literal){archive_dir_str}"),
    ],
    STATUS_PROBE_BYTES,
  )?;
  if truncated || !tracked.is_empty() {
    return Ok(None);
  }
  let entries = match std::fs::read_dir(state_root.join("code-review-usage")) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Some(Vec::new())),
    Err(_) => return Ok(None),
  };
  let mut excludes = Vec::new();
  for entry in entries {
    let Ok(entry) = entry else { return Ok(None) };
    let name = entry.file_name();
    let Some(name) = name.to_str() else {
      return Ok(None);
    };
    let Some(digits) = name.strip_suffix(".jsonl") else {
      continue;
    };
    if digits.len() == 20
      && digits.bytes().all(|byte| byte.is_ascii_digit())
      && digits.parse::<u64>().is_ok()
      && entry.file_type().is_ok_and(|kind| kind.is_file())
    {
      excludes.push(format!(":(exclude,literal){archive_dir_str}/{name}"));
    }
  }
  Ok(Some(excludes))
}

fn dirty_with_excludes(env: &Env, root: &Path, excludes: &[String]) -> Result<bool, ShapeError> {
  let mut args = vec![
    "--no-optional-locks",
    "status",
    "--porcelain=v1",
    "-z",
    "--untracked-files=all",
    "--",
    ":/",
  ];
  args.extend(excludes.iter().map(String::as_str));
  let (status, truncated) = git_output(env, root, &args, STATUS_PROBE_BYTES)?;
  Ok(truncated || !status.is_empty())
}

#[cfg(test)]
#[path = "tests/dirty_test.rs"]
mod tests;
