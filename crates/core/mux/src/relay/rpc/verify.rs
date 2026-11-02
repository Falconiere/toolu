//! Verify incoming bundles before accepting a durable task.

use crate::{hc::remote::import_recover, relay::store::Submission};
use std::path::Path;
use toolu_runtime::{
  env::Env,
  process::{Spec, run},
};

/// Import the exact advertised input commit before acknowledging submission.
pub(super) fn input(
  root: &Path,
  env: &Env,
  request: &Submission,
  bytes: &[u8],
) -> Result<(), String> {
  if request.key.is_empty()
    || request.key.len() > 128
    || !request
      .key
      .bytes()
      .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
  {
    return Err("invalid task key".into());
  }
  let repo = root.join("inputs").join(&request.key);
  crate::relay::durable::ensure_dir(&repo)?;
  let path = repo.to_str().ok_or("non-UTF-8 input repository path")?;
  let mut spec = Spec::new(["git", "init", "--bare", "--quiet", path]);
  spec.env = Some(env.clone());
  let output = run(&spec).map_err(|e| format!("initializing input repository: {e:?}"))?;
  if output.exit_code != 0 || output.timed_out || output.truncated {
    return Err("initializing input repository failed".into());
  }
  import_recover(env, path, &request.key, &request.base, bytes)
    .map_err(|e| format!("invalid input bundle: {}", e.message))
}

#[cfg(test)]
#[path = "tests/verify_test.rs"]
mod tests;
