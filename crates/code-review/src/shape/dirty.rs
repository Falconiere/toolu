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
    let Some(archives) = prefix
      .join("code-review-usage/*.jsonl")
      .to_str()
      .map(str::to_owned)
    else {
      return Ok(true);
    };
    excludes.push(format!(":(exclude,glob){archives}"));
  }
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
