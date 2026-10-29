//! Explicit, recoverable transfers between existing Git checkouts.

use super::{Error, process_error, uncertain};
use std::path::{Component, Path};
use std::time::Duration;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

mod chunk;
mod local;
mod preflight;
mod recover;
pub use local::{export, import};
pub use recover::{export_recover, import_recover};

const LIMIT: usize = 8 * 1024 * 1024;
const BUNDLE_LIMIT: usize = 512 * 1024 * 1024;

/// A configured hc machine and absolute checkout path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target<'a> {
  /// Configured hc machine name.
  pub machine: &'a str,
  /// Absolute machine-side checkout path.
  pub repo: &'a str,
}

impl<'a> Target<'a> {
  /// Validate the paired settings. Both absent means local execution.
  ///
  /// # Errors
  /// Only one setting is present, or either setting is unsafe.
  pub fn new(machine: Option<&'a str>, repo: Option<&'a str>) -> Result<Option<Self>, Error> {
    match (machine, repo) {
      (None, None) => Ok(None),
      (Some(machine), Some(repo)) if safe(machine) && absolute(repo) => {
        Ok(Some(Self { machine, repo }))
      }
      _ => Err(Error::new(
        "invalid_target",
        "machine and absolute repository path must be supplied together",
      )),
    }
  }
}

fn safe(value: &str) -> bool {
  !value.is_empty()
    && value.len() <= 128
    && value
      .bytes()
      .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn absolute(value: &str) -> bool {
  let path = Path::new(value);
  path.is_absolute()
    && !value.contains('\0')
    && path
      .components()
      .all(|c| !matches!(c, Component::ParentDir | Component::CurDir))
}

fn commit(value: &str) -> bool {
  (value.len() == 40 || value.len() == 64)
    && value
      .bytes()
      .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// A run-scoped transport. Callers must persist the unique run ID before use.
pub struct Transfer<'a> {
  /// Existing remote checkout.
  pub target: Target<'a>,
  /// Explicit subprocess environment.
  pub env: &'a Env,
  /// Unique, durable run identity.
  pub run: &'a str,
}

impl Transfer<'_> {
  fn check(&self) -> Result<(), Error> {
    if Target::new(Some(self.target.machine), Some(self.target.repo))?.is_none() || !safe(self.run)
    {
      return Err(Error::new("invalid_run", "invalid transfer run ID"));
    }
    Ok(())
  }

  /// Execute argv on the configured machine without a shell, passing exact stdin bytes.
  ///
  /// # Errors
  /// The command fails, times out, or exceeds the output limit.
  pub fn exec(&self, program: &[&str], input: &[u8]) -> Result<Vec<u8>, Error> {
    self.check()?;
    if input.len() > LIMIT {
      return Err(Error::new("too_large", "transfer exceeds 8 MiB"));
    }
    let mut args = vec![
      "hc",
      "exec",
      "--on",
      self.target.machine,
      "--cwd",
      self.target.repo,
      "--timeout",
      "120",
      "-i",
      "--",
    ];
    args.extend_from_slice(program);
    command(self.env, &args, input)
  }

  /// Execute Git in the existing remote checkout.
  ///
  /// # Errors
  /// Git fails or returns non-UTF-8 output.
  pub fn git(&self, parts: &[&str]) -> Result<String, Error> {
    let mut argv = vec!["git"];
    argv.extend_from_slice(parts);
    text(self.exec(&argv, &[])?)
  }

  /// Return the absolute remote path for a transfer file.
  ///
  /// # Errors
  /// The name, run, or remote Git directory is invalid.
  pub fn path(&self, name: &str) -> Result<String, Error> {
    self.check()?;
    if !safe(name) {
      return Err(Error::new("invalid_name", "invalid transfer file name"));
    }
    let git_dir = self.git(&["rev-parse", "--absolute-git-dir"])?;
    if !absolute(&git_dir) {
      return Err(Error::new("invalid_output", "invalid remote git directory"));
    }
    Ok(format!(
      "{git_dir}/toolu-transfer/{}/{name}/content",
      self.run
    ))
  }

  /// Allocate the run directory once. Never reuse an existing directory.
  ///
  /// # Errors
  /// The directory exists or cannot be created.
  pub fn allocate(&self) -> Result<(), Error> {
    let file = self.path("placeholder")?;
    let dir = Path::new(&file)
      .parent()
      .and_then(Path::parent)
      .and_then(Path::to_str)
      .ok_or_else(|| Error::new("invalid_output", "invalid transfer directory"))?;
    let parent = Path::new(dir)
      .parent()
      .and_then(Path::to_str)
      .ok_or_else(|| Error::new("invalid_output", "invalid transfer directory"))?;
    self.exec(&["mkdir", "-p", parent], &[])?;
    self.exec(&["mkdir", dir], &[])?;
    Ok(())
  }

  /// Write a named file to a newly allocated run directory.
  ///
  /// # Errors
  /// Input exceeds hc's limit or the remote write fails.
  pub fn write(&self, name: &str, bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > LIMIT {
      return Err(Error::new("too_large", "transfer exceeds 8 MiB"));
    }
    let path = self.path(name)?;
    let dir = Path::new(&path)
      .parent()
      .and_then(Path::to_str)
      .ok_or_else(|| Error::new("invalid_output", "invalid transfer path"))?;
    self.exec(&["mkdir", dir], &[])?;
    let args = ["hc", "file", "write", "--on", self.target.machine, &path];
    command(self.env, &args, bytes)?;
    Ok(())
  }

  /// Read the exact bytes of a named transfer file.
  ///
  /// # Errors
  /// The file cannot be read or exceeds the transport limit.
  pub fn read(&self, name: &str) -> Result<Vec<u8>, Error> {
    let path = self.path(name)?;
    self.exec(&["cat", "--", &path], &[])
  }

  /// Create a remote bundle from a committed snapshot in the allocated run directory.
  ///
  /// # Errors
  /// The run, commit, destination, or existing private ref is invalid.
  pub fn export(&self, name: &str, expected: &str) -> Result<Vec<u8>, Error> {
    if !commit(expected) {
      return Err(Error::new("invalid_commit", "expected full commit ID"));
    }
    let path = self.path(name)?;
    let dir = Path::new(&path)
      .parent()
      .and_then(Path::to_str)
      .ok_or_else(|| Error::new("invalid_output", "invalid transfer path"))?;
    let resolved = self.git(&["rev-parse", &format!("{expected}^{{commit}}")])?;
    if resolved != expected {
      return Err(Error::new("identity_mismatch", "commit differs"));
    }
    self.exec(&["mkdir", dir], &[])?;
    let reference = format!("refs/toolu-transfer/{}/snapshot", self.run);
    self.git(&["update-ref", &reference, expected, ""])?;
    self.git(&["bundle", "create", &path, &reference])?;
    self.git(&["bundle", "verify", &path])?;
    verify_heads(
      &self.git(&["bundle", "list-heads", &path])?,
      expected,
      &reference,
    )?;
    self.read_bundle(name)
  }

  /// Import a previously uploaded bundle under a private ref after verifying its commit.
  ///
  /// # Errors
  /// The bundle or its advertised ref differs, or Git cannot verify/import it.
  pub fn import(&self, name: &str, expected: &str) -> Result<(), Error> {
    if !commit(expected) {
      return Err(Error::new("invalid_commit", "expected full commit ID"));
    }
    let path = self.path(name)?;
    let reference = format!("refs/toolu-transfer/{}/snapshot", self.run);
    self.git(&["bundle", "verify", &path])?;
    verify_heads(
      &self.git(&["bundle", "list-heads", &path])?,
      expected,
      &reference,
    )?;
    self.git(&[
      "fetch",
      "--no-write-fetch-head",
      "--no-tags",
      &path,
      &reference,
    ])?;
    self.git(&["update-ref", &reference, expected, ""])?;
    if self.git(&["rev-parse", &reference])? != expected {
      return Err(Error::new("identity_mismatch", "imported commit differs"));
    }
    Ok(())
  }
}

fn command(env: &Env, args: &[&str], input: &[u8]) -> Result<Vec<u8>, Error> {
  let mut spec = Spec::new(args.iter().copied());
  spec.env = Some(env.clone());
  spec.stdin = input.to_vec();
  spec.timeout = Duration::from_secs(130);
  spec.max_output_bytes = LIMIT + 1;
  let out = run(&spec).map_err(|error| process_error(&error))?;
  if out.timed_out || out.truncated {
    return Err(Error {
      code: "transport".into(),
      message: "command timed out or output exceeded limit".into(),
      uncertain: true,
    });
  }
  if out.exit_code != 0 {
    return Err(uncertain(Error::new(
      "command_failed",
      format!("command exited {}", out.exit_code),
    )));
  }
  if out.stdout_bytes.len() > LIMIT {
    return Err(Error::new("too_large", "transfer exceeds 8 MiB"));
  }
  Ok(out.stdout_bytes)
}

fn text(bytes: Vec<u8>) -> Result<String, Error> {
  String::from_utf8(bytes)
    .map(|s| s.trim_end_matches(['\r', '\n']).to_owned())
    .map_err(|_error| Error::new("invalid_output", "command returned non-UTF-8 text"))
}

fn verify_heads(heads: &str, expected: &str, reference: &str) -> Result<(), Error> {
  if heads != format!("{expected} {reference}") {
    return Err(Error::new(
      "identity_mismatch",
      "bundle ref or commit differs",
    ));
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/remote_test.rs"]
mod tests;
