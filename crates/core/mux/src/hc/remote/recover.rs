//! Recover interrupted transfers without replacing existing evidence.

use super::{BUNDLE_LIMIT, Error, Transfer, commit, local::local_git, safe, verify_heads};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use toolu_runtime::env::Env;

fn identity(run: &str, expected: &str) -> Result<String, Error> {
  if !safe(run) || !commit(expected) {
    return Err(Error::new("invalid_identity", "invalid run or commit"));
  }
  Ok(format!("refs/toolu-transfer/{run}/snapshot"))
}

fn check_ref(output: &str, reference: &str, expected: &str) -> Result<bool, Error> {
  if output.is_empty() {
    return Ok(false);
  }
  if output != format!("{reference} {expected}") {
    return Err(Error::new("identity_mismatch", "transfer ref differs"));
  }
  Ok(true)
}

fn local_ref(env: &Env, repo: &str, reference: &str, expected: &str) -> Result<bool, Error> {
  let output = local_git(
    env,
    repo,
    &[
      "for-each-ref",
      "--format=%(refname) %(objectname)",
      reference,
    ],
  )?;
  check_ref(&output, reference, expected)
}

fn path(env: &Env, repo: &str, run: &str) -> Result<PathBuf, Error> {
  let git_dir = local_git(env, repo, &["rev-parse", "--absolute-git-dir"])?;
  let dir = Path::new(&git_dir).join("toolu-transfer").join(run);
  std::fs::create_dir_all(
    dir
      .parent()
      .ok_or_else(|| Error::new("io", "invalid git directory"))?,
  )
  .map_err(|e| Error::new("io", e.to_string()))?;
  match std::fs::create_dir(&dir) {
    Ok(()) => (),
    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
      let metadata =
        std::fs::symlink_metadata(&dir).map_err(|e| Error::new("io", e.to_string()))?;
      if !metadata.is_dir() {
        return Err(Error::new(
          "identity_mismatch",
          "transfer directory is not a directory",
        ));
      }
    }
    Err(e) => return Err(Error::new("io", e.to_string())),
  }
  Ok(dir.join("snapshot.bundle"))
}

fn local_file(path: &Path, bytes: &[u8]) -> Result<(), Error> {
  if bytes.len() > BUNDLE_LIMIT {
    return Err(Error::new("too_large", "bundle exceeds 512 MiB"));
  }
  match std::fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(path)
  {
    Ok(mut file) => file
      .write_all(bytes)
      .map_err(|e| Error::new("io", e.to_string())),
    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
      let metadata =
        std::fs::symlink_metadata(path).map_err(|e| Error::new("io", e.to_string()))?;
      if !metadata.is_file()
        || std::fs::read(path).map_err(|e| Error::new("io", e.to_string()))? != bytes
      {
        return Err(Error::new("identity_mismatch", "transfer bundle differs"));
      }
      Ok(())
    }
    Err(e) => Err(Error::new("io", e.to_string())),
  }
}

fn verify_local(
  env: &Env,
  repo: &str,
  path: &Path,
  reference: &str,
  expected: &str,
) -> Result<(), Error> {
  let file = path
    .to_str()
    .ok_or_else(|| Error::new("invalid_output", "non-UTF-8 path"))?;
  local_git(env, repo, &["bundle", "verify", file])?;
  verify_heads(
    &local_git(env, repo, &["bundle", "list-heads", file])?,
    expected,
    reference,
  )
}

/// Resume an export, retaining and verifying any existing ref and bundle.
///
/// # Errors
/// An existing artifact differs, or Git or file operations fail.
pub fn export_recover(env: &Env, repo: &str, run: &str, expected: &str) -> Result<Vec<u8>, Error> {
  let reference = identity(run, expected)?;
  let path = path(env, repo, run)?;
  if !local_ref(env, repo, &reference, expected)? {
    local_git(env, repo, &["update-ref", &reference, expected, ""])?;
  }
  if !path.exists() {
    let build = path
      .parent()
      .ok_or_else(|| Error::new("io", "invalid bundle path"))?
      .join("build");
    std::fs::create_dir(&build).map_err(|e| Error::new("io", e.to_string()))?;
    let candidate = build.join("bundle");
    let file = candidate
      .to_str()
      .ok_or_else(|| Error::new("invalid_output", "non-UTF-8 path"))?;
    local_git(env, repo, &["bundle", "create", file, &reference])?;
    verify_local(env, repo, &candidate, &reference, expected)?;
    std::fs::hard_link(&candidate, &path).map_err(|e| Error::new("io", e.to_string()))?;
  }
  let metadata = std::fs::symlink_metadata(&path).map_err(|e| Error::new("io", e.to_string()))?;
  if !metadata.is_file() || metadata.len() > BUNDLE_LIMIT as u64 {
    return Err(Error::new("identity_mismatch", "invalid bundle file"));
  }
  verify_local(env, repo, &path, &reference, expected)?;
  std::fs::read(path).map_err(|e| Error::new("io", e.to_string()))
}

/// Resume an import, requiring existing bundle bytes and ref to match exactly.
///
/// # Errors
/// An artifact differs, the bundle is invalid, or Git or file operations fail.
pub fn import_recover(
  env: &Env,
  repo: &str,
  run: &str,
  expected: &str,
  bytes: &[u8],
) -> Result<(), Error> {
  let reference = identity(run, expected)?;
  let path = path(env, repo, run)?;
  let existing = local_ref(env, repo, &reference, expected)?;
  local_file(&path, bytes)?;
  verify_local(env, repo, &path, &reference, expected)?;
  if !existing {
    let file = path
      .to_str()
      .ok_or_else(|| Error::new("invalid_output", "non-UTF-8 path"))?;
    local_git(
      env,
      repo,
      &[
        "fetch",
        "--no-write-fetch-head",
        "--no-tags",
        file,
        &reference,
      ],
    )?;
    local_git(env, repo, &["update-ref", &reference, expected, ""])?;
  }
  Ok(())
}

impl Transfer<'_> {
  pub(super) fn ensure_run(&self) -> Result<(), Error> {
    let path = self.path("placeholder")?;
    let run = Path::new(&path)
      .parent()
      .and_then(Path::parent)
      .and_then(Path::to_str)
      .ok_or_else(|| Error::new("invalid_output", "invalid transfer directory"))?;
    self.exec(&["mkdir", "-p", run], &[])?;
    Ok(())
  }

  fn ref_exists(&self, reference: &str, expected: &str) -> Result<bool, Error> {
    check_ref(
      &self.git(&[
        "for-each-ref",
        "--format=%(refname) %(objectname)",
        reference,
      ])?,
      reference,
      expected,
    )
  }

  fn verified(&self, name: &str, reference: &str, expected: &str) -> Result<(), Error> {
    let file = self.path(name)?;
    self.git(&["bundle", "verify", &file])?;
    verify_heads(
      &self.git(&["bundle", "list-heads", &file])?,
      expected,
      reference,
    )
  }

  pub(super) fn put_once(&self, name: &str, bytes: &[u8]) -> Result<(), Error> {
    match self.write(name, bytes) {
      Ok(()) => Ok(()),
      Err(err) => match self.read(name) {
        Ok(found) if found == bytes => Ok(()),
        Ok(_) => Err(Error::new("identity_mismatch", "transfer file differs")),
        Err(_) => Err(err),
      },
    }
  }

  /// Resume upload and import, matching existing bytes and private ref exactly.
  ///
  /// # Errors
  /// An existing artifact differs, or the transfer or verification fails.
  pub fn upload(&self, name: &str, expected: &str, bytes: &[u8]) -> Result<(), Error> {
    let reference = identity(self.run, expected)?;
    self.ensure_run()?;
    let existing = self.ref_exists(&reference, expected)?;
    self.put_bundle(name, bytes)?;
    self.verified(name, &reference, expected)?;
    if !existing {
      self.import(name, expected)?;
    }
    Ok(())
  }

  /// Resume export without replacing the private ref or bundle.
  ///
  /// # Errors
  /// An existing artifact differs, or Git or the transfer fails.
  pub fn export_recover(&self, name: &str, expected: &str) -> Result<Vec<u8>, Error> {
    let reference = identity(self.run, expected)?;
    self.ensure_run()?;
    if !self.ref_exists(&reference, expected)? {
      self.git(&["update-ref", &reference, expected, ""])?;
    }
    self.create_bundle(name, &reference)?;
    self.verified(name, &reference, expected)?;
    self.read_bundle(name)
  }
}

#[cfg(test)]
#[path = "tests/recover_test.rs"]
mod tests;
