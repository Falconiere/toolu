//! Bounded bundle transfers using run-scoped chunk files.

use super::{BUNDLE_LIMIT, Error, Transfer};
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
    let size = String::from_utf8(self.exec(&["wc", "-c", &path], &[])?)
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

  pub(super) fn put_bundle(&self, name: &str, bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > BUNDLE_LIMIT {
      return Err(Error::new("too_large", "bundle exceeds 512 MiB"));
    }
    for (index, chunk) in bytes.chunks(CHUNK).enumerate() {
      self.put_once(&format!("{name}-chunk-{index}"), chunk)?;
    }
    let (dir, path) = self.bundle_dir(name)?;
    if self.exec(&["mkdir", &dir], &[]).is_err() {
      return if self.read_bundle(name)? == bytes {
        Ok(())
      } else {
        Err(Error::new("identity_mismatch", "bundle differs"))
      };
    }
    for index in 0..bytes.len().div_ceil(CHUNK) {
      let chunk = self.path(&format!("{name}-chunk-{index}"))?;
      self.exec(
        &[
          "dd",
          &format!("if={chunk}"),
          &format!("of={path}"),
          "bs=4194304",
          &format!("seek={index}"),
          "conv=notrunc",
        ],
        &[],
      )?;
    }
    if self.read_bundle(name)? != bytes {
      return Err(Error::new("identity_mismatch", "assembled bundle differs"));
    }
    Ok(())
  }

  pub(super) fn create_bundle(&self, name: &str, reference: &str) -> Result<(), Error> {
    let (dir, path) = self.bundle_dir(name)?;
    if self.exec(&["mkdir", &dir], &[]).is_ok() {
      self.git(&["bundle", "create", &path, reference])?;
    }
    Ok(())
  }
}

#[cfg(test)]
#[path = "tests/chunk_test.rs"]
mod tests;
