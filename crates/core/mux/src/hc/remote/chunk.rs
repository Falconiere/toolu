//! Bounded bundle transfers using run-scoped chunk files.

use super::{BUNDLE_LIMIT, Error, Transfer, command, text};
use std::path::Path;

const CHUNK: usize = 4 * 1024 * 1024;

impl Transfer<'_> {
  fn bundle_dir(&self, name: &str) -> Result<(String, String), Error> {
    let path = self.path(name)?;
    let dir = Path::new(&path)
      .parent()
      .and_then(Path::to_str)
      .ok_or_else(|| Error::new("invalid_output", "invalid bundle path"))?;
    Ok((dir.to_owned(), path))
  }

  /// Read a bundle in bounded blocks, preserving its exact bytes.
  ///
  /// # Errors
  /// The file is unreadable or exceeds the bundle limit.
  pub fn read_bundle(&self, name: &str) -> Result<Vec<u8>, Error> {
    let path = self.path(name)?;
    self.read_file(&path)
  }

  fn read_file(&self, path: &str) -> Result<Vec<u8>, Error> {
    let size = String::from_utf8(self.exec(&["wc", "-c", path], &[])?)
      .map_err(|_error| Error::new("invalid_output", "invalid file size"))?;
    let size = size
      .split_whitespace()
      .next()
      .and_then(|s| s.parse::<usize>().ok())
      .ok_or_else(|| Error::new("invalid_output", "invalid file size"))?;
    if size > BUNDLE_LIMIT {
      return Err(Error::new("too_large", "bundle exceeds 512 MiB"));
    }
    let mut result = Vec::with_capacity(size);
    for index in 0..size.div_ceil(CHUNK) {
      let block = self.exec(
        &[
          "dd",
          &format!("if={path}"),
          "bs=4194304",
          &format!("skip={index}"),
          "count=1",
        ],
        &[],
      )?;
      let expected = CHUNK.min(size - result.len());
      if block.len() != expected {
        return Err(Error::new("invalid_output", "short bundle read"));
      }
      result.extend_from_slice(&block);
    }
    Ok(result)
  }

  fn staging(&self, dir: &str) -> Result<String, Error> {
    self.exec(&["mkdir", "-p", dir], &[])?;
    let name = text(self.exec(&["mktemp", "-d", &format!("{dir}/build-XXXXXXXX")], &[])?)?;
    Ok(format!("{name}/content"))
  }

  fn has_content(&self, dir: &str) -> Result<bool, Error> {
    let listing = text(self.exec(&["ls", "-A", dir], &[])?)?;
    Ok(listing.lines().any(|line| line == "content"))
  }

  fn publish(&self, stage: &str, path: &str, previous: Option<&[u8]>) -> Result<(), Error> {
    if let Some(previous) = previous {
      let parent = Path::new(stage)
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(|| Error::new("invalid_output", "invalid stage path"))?;
      let backup = format!("{parent}/previous");
      self.exec(&["mv", path, &backup], &[])?;
      if self.read_file(&backup)? != previous {
        return Err(Error::new(
          "identity_mismatch",
          "partial file changed during recovery",
        ));
      }
    }
    self.exec(&["ln", stage, path], &[])?;
    Ok(())
  }

  pub(super) fn put_once(&self, name: &str, bytes: &[u8]) -> Result<(), Error> {
    let (dir, path) = self.bundle_dir(name)?;
    let stage = self.staging(&dir)?;
    let current = if self.has_content(&dir)? {
      Some(self.read(name)?)
    } else {
      None
    };
    if let Some(current) = &current {
      if !bytes.starts_with(current) {
        return Err(Error::new("identity_mismatch", "transfer file differs"));
      }
      if *current == bytes {
        return Ok(());
      }
    }
    let args = ["hc", "file", "write", "--on", self.target.machine, &stage];
    command(self.env, &args, bytes)?;
    self.publish(&stage, &path, current.as_deref())?;
    if self.read(name)? != bytes {
      return Err(Error::new("identity_mismatch", "transfer file differs"));
    }
    Ok(())
  }

  pub(super) fn put_bundle(&self, name: &str, bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > BUNDLE_LIMIT {
      return Err(Error::new("too_large", "bundle exceeds 512 MiB"));
    }
    for (index, chunk) in bytes.chunks(CHUNK).enumerate() {
      self.put_once(&format!("{name}-chunk-{index}"), chunk)?;
    }
    let (dir, path) = self.bundle_dir(name)?;
    let stage = self.staging(&dir)?;
    let current = if self.has_content(&dir)? {
      Some(self.read_bundle(name)?)
    } else {
      None
    };
    if let Some(current) = &current {
      if !bytes.starts_with(current) {
        return Err(Error::new("identity_mismatch", "bundle differs"));
      }
      if *current == bytes {
        return Ok(());
      }
    }
    for index in 0..bytes.len().div_ceil(CHUNK) {
      let chunk = self.path(&format!("{name}-chunk-{index}"))?;
      self.exec(
        &[
          "dd",
          &format!("if={chunk}"),
          &format!("of={stage}"),
          "bs=4194304",
          &format!("seek={index}"),
          "conv=notrunc",
        ],
        &[],
      )?;
    }
    if self.read_file(&stage)? != bytes {
      return Err(Error::new("identity_mismatch", "assembled bundle differs"));
    }
    self.publish(&stage, &path, current.as_deref())?;
    Ok(())
  }

  pub(super) fn create_bundle(&self, name: &str, reference: &str) -> Result<(), Error> {
    let (dir, _) = self.bundle_dir(name)?;
    let stage = self.staging(&dir)?;
    self.git(&["bundle", "create", &stage, reference])?;
    let bytes = self.read_file(&stage)?;
    self.put_bundle(name, &bytes)
  }
}

#[cfg(test)]
#[path = "tests/chunk_test.rs"]
mod tests;
