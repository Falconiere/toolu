//! Local half of the committed bundle transfer.

use super::{BUNDLE_LIMIT, Error, command, commit, safe, text, verify_heads};
use std::path::Path;
use toolu_runtime::env::Env;

/// Export a committed snapshot into a new private run directory and return its bytes.
///
/// # Errors
/// The run or commit is invalid, already exists, or Git cannot create the bundle.
pub fn export(env: &Env, repo: &str, run: &str, expected: &str) -> Result<Vec<u8>, Error> {
  if !safe(run) || !commit(expected) {
    return Err(Error::new("invalid_identity", "invalid run or commit ID"));
  }
  let resolved = local_git(env, repo, &["rev-parse", &format!("{expected}^{{commit}}")])?;
  if resolved != expected {
    return Err(Error::new("identity_mismatch", "commit differs"));
  }
  let dir = local_dir(env, repo, run)?;
  let path = dir.join("snapshot.bundle");
  let path = path
    .to_str()
    .ok_or_else(|| Error::new("invalid_output", "non-UTF-8 path"))?;
  let reference = format!("refs/toolu-transfer/{run}/snapshot");
  local_git(env, repo, &["update-ref", &reference, expected, ""])?;
  local_git(env, repo, &["bundle", "create", path, &reference])?;
  local_git(env, repo, &["bundle", "verify", path])?;
  verify_heads(
    &local_git(env, repo, &["bundle", "list-heads", path])?,
    expected,
    &reference,
  )?;
  let size = std::fs::metadata(path)
    .map_err(|e| Error::new("io", e.to_string()))?
    .len();
  if size > BUNDLE_LIMIT as u64 {
    return Err(Error::new("too_large", "bundle exceeds 512 MiB"));
  }
  std::fs::read(path).map_err(|e| Error::new("io", e.to_string()))
}

/// Import a downloaded bundle only after checking its private ref and commit.
///
/// # Errors
/// The run already exists, the bundle is invalid, or Git rejects the import.
pub fn import(env: &Env, repo: &str, run: &str, expected: &str, bytes: &[u8]) -> Result<(), Error> {
  if !safe(run) || !commit(expected) || bytes.len() > BUNDLE_LIMIT {
    return Err(Error::new(
      "invalid_identity",
      "invalid run, commit, or bundle size",
    ));
  }
  let dir = local_dir(env, repo, run)?;
  let path = dir.join("snapshot.bundle");
  std::fs::write(&path, bytes).map_err(|e| Error::new("io", e.to_string()))?;
  let path = path
    .to_str()
    .ok_or_else(|| Error::new("invalid_output", "non-UTF-8 path"))?;
  let reference = format!("refs/toolu-transfer/{run}/snapshot");
  local_git(env, repo, &["bundle", "verify", path])?;
  verify_heads(
    &local_git(env, repo, &["bundle", "list-heads", path])?,
    expected,
    &reference,
  )?;
  local_git(
    env,
    repo,
    &[
      "fetch",
      "--no-write-fetch-head",
      "--no-tags",
      path,
      &reference,
    ],
  )?;
  local_git(env, repo, &["update-ref", &reference, expected, ""])?;
  if local_git(env, repo, &["rev-parse", &reference])? != expected {
    return Err(Error::new("identity_mismatch", "imported commit differs"));
  }
  Ok(())
}

fn local_dir(env: &Env, repo: &str, run: &str) -> Result<std::path::PathBuf, Error> {
  let git_dir = local_git(env, repo, &["rev-parse", "--absolute-git-dir"])?;
  let dir = Path::new(&git_dir).join("toolu-transfer").join(run);
  std::fs::create_dir_all(
    dir
      .parent()
      .ok_or_else(|| Error::new("invalid_output", "invalid git directory"))?,
  )
  .map_err(|e| Error::new("io", e.to_string()))?;
  std::fs::create_dir(&dir).map_err(|e| Error::new("io", e.to_string()))?;
  Ok(dir)
}

pub(super) fn local_git(env: &Env, repo: &str, parts: &[&str]) -> Result<String, Error> {
  let mut argv = vec!["git", "-C", repo];
  argv.extend_from_slice(parts);
  text(command(env, &argv, &[])?)
}

#[cfg(test)]
#[path = "tests/local_test.rs"]
mod tests;
