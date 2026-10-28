//! Ignore only this plugin's own untracked state in the dirty-worktree probe.

use std::path::Path;
use toolu_runtime::env::Env;

use super::git_io::git_output;
use super::{STATUS_PROBE_BYTES, ShapeError};

/// Whether Git sees changes besides the review plugin's own untracked state files.
pub(crate) fn dirty_except_state(
  env: &Env,
  root: &Path,
  state_root: &Path,
) -> Result<bool, ShapeError> {
  let Ok(prefix) = state_root.strip_prefix(root) else {
    return Ok(true);
  };
  let Some(prefix) = prefix.to_str() else {
    return Ok(true);
  };
  let (status, truncated) = git_output(
    env,
    root,
    &[
      "--no-optional-locks",
      "status",
      "--porcelain=v1",
      "-z",
      "--untracked-files=all",
    ],
    STATUS_PROBE_BYTES,
  )?;
  if truncated {
    return Ok(true);
  }
  let prefix = format!("{prefix}/");
  for entry in status
    .split(|byte| *byte == 0)
    .filter(|entry| !entry.is_empty())
  {
    let Some(path) = entry.strip_prefix(b"?? ") else {
      return Ok(true);
    };
    let Ok(path) = std::str::from_utf8(path) else {
      return Ok(true);
    };
    let Some(state_path) = path.strip_prefix(&prefix) else {
      return Ok(true);
    };
    if !state_path.starts_with("code-review/")
      && state_path != "code-review-usage.jsonl"
      && !state_path.starts_with("push-review/")
    {
      return Ok(true);
    }
  }
  Ok(false)
}

#[cfg(test)]
#[path = "tests/dirty_test.rs"]
mod tests;
