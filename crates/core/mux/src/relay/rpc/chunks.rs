use super::{CHUNK, LIMIT};
use crate::relay::durable;
use std::path::{Path, PathBuf};
use std::time::Duration;
use toolu_state::lock::{LockOptions, with_lock_strict};

fn directory(root: &Path, key: &str) -> Result<PathBuf, String> {
  if !crate::relay::store::safe(key) {
    return Err("invalid key".into());
  }
  Ok(root.join("inbox").join(key))
}

pub(super) fn put(root: &Path, key: &str, index: u64, bytes: &[u8]) -> Result<(), String> {
  if bytes.is_empty() || bytes.len() > CHUNK || index >= (LIMIT / CHUNK) as u64 {
    return Err("invalid chunk".into());
  }
  let dir = directory(root, key)?;
  durable::ensure_dir(&dir)?;
  let path = dir.join(format!("chunk-{index}"));
  let options = LockOptions {
    stale: Duration::MAX,
    ..LockOptions::default()
  };
  with_lock_strict(&path, options, || {
    if path.exists() {
      if std::fs::read(&path).map_err(|e| e.to_string())? != bytes {
        return Err("chunk conflicts with existing bytes".into());
      }
      return Ok(());
    }
    durable::write(&path, bytes)
  })?
}

pub(super) fn assemble(root: &Path, key: &str, count: u64, size: u64) -> Result<Vec<u8>, String> {
  let size = usize::try_from(size).map_err(|_error| "invalid size")?;
  if size == 0 || size > LIMIT || count != size.div_ceil(CHUNK) as u64 {
    return Err("invalid chunk count or size".into());
  }
  let dir = directory(root, key)?;
  let mut result = Vec::with_capacity(size);
  for index in 0..count {
    let path = dir.join(format!("chunk-{index}"));
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() != CHUNK.min(size - result.len()) {
      return Err("invalid chunk size".into());
    }
    result.extend_from_slice(&bytes);
  }
  Ok(result)
}

#[cfg(test)]
#[path = "tests/chunks_test.rs"]
mod tests;
